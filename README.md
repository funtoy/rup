# rup

一个基于 SSH 的远程部署小工具:连接远程服务器,上传文件(带实时进度条),并按顺序执行命令。适合把「上传 + 解压 + 重启服务」这类日常部署流程压缩成一条命令。

## 功能

- **多种认证方式**:密码、私钥文件(支持 passphrase)、SSH agent
- **文件上传**:通过 SFTP 上传文件,并轮询远程文件大小显示真实上传进度(进度条含速率、百分比)
- **依赖保障**:`--ensure` 检查远程工具是否已安装,缺失时自动通过 `apt-get` / `yum` / `apk` 安装
- **远程执行**:`--exec` 可多次指定,按顺序在远程执行命令
- **工作目录**:`--work-dir` 自动创建目录,上传与命令执行都在该目录下进行

## 安装

需要 [Rust 工具链](https://rustup.rs/)。

```bash
cargo build --release
# 产物在 target/release/rup
```

## 使用

```bash
rup --host <IP> [选项]
```

### 参数

| 参数 | 说明 | 默认值 |
| --- | --- | --- |
| `--host` | 服务器地址(必填) | — |
| `--port` | SSH 端口 | `22` |
| `--user` | 用户名 | `root` |
| `--password` | 密码(建议改用环境变量 `RUP_PASSWORD`) | — |
| `--key-file` | 私钥文件路径 | — |
| `--key-passphrase` | 私钥 passphrase(也可用环境变量 `RUP_KEY_PASSPHRASE`) | — |
| `--use-agent` | 使用 SSH agent 认证(仅 Unix) | — |
| `--upload-file` | 要上传的本地文件 | — |
| `--work-dir` | 远程工作目录(会自动 `mkdir -p`) | — |
| `--ensure` | 确保远程工具已安装,可多次指定 | — |
| `--exec` | 要执行的远程命令,可多次指定、按顺序执行 | — |

认证方式三选一,优先级为 `--use-agent` > `--key-file` > `--password`。

### 示例

上传压缩包并解压、重启服务:

```bash
export RUP_PASSWORD='your-password'

rup --host 192.168.1.100 \
    --user root \
    --work-dir /opt/myapp \
    --ensure unzip \
    --upload-file dist.zip \
    --exec 'unzip -o dist.zip' \
    --exec 'systemctl restart myapp'
```

使用私钥认证:

```bash
rup --host example.com --key-file ~/.ssh/id_ed25519 \
    --upload-file app.tar.gz \
    --work-dir /srv/app \
    --exec 'tar xzf app.tar.gz'
```

使用 SSH agent:

```bash
rup --host example.com --use-agent --exec 'uptime'
```

## 执行顺序

1. 建立 SSH 连接
2. `--ensure` 检查并安装缺失的工具
3. `--work-dir` 创建工作目录
4. `--upload-file` 上传文件(上传到工作目录下,显示进度条)
5. `--exec` 依次执行命令(在工作目录下)

## 注意事项

- 主机密钥校验目前为 `NoCheck`(不校验服务器指纹),请只在可信网络环境中使用。
- 命令行传入 `--password` 会出现在进程列表中,推荐使用环境变量 `RUP_PASSWORD`。
- 自动安装依赖需要远程用户具有包管理器权限(通常为 root)。
- 依赖版本说明:`russh-sftp` 锁定在 `2.1.1`,因为 `async-ssh2-tokio 0.12.2` 依赖其 `new_opts` 方法,`2.3.0` 已移除,升级会导致编译失败。

## License

MIT
