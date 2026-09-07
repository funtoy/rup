use async_ssh2_tokio::client::{AuthMethod, Client, ServerCheckMethod};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// IP地址
    #[arg(long)]
    host: String,

    /// 端口
    #[arg(long, default_value_t = 22)]
    port: u16,

    /// 用户名
    #[arg(long, default_value = "root")]
    user: String,

    /// 密码（也可通过环境变量 RUP_PASSWORD 设置，避免在进程列表中暴露）
    #[arg(long)]
    password: Option<String>,

    /// 证书文件
    #[arg(long)]
    key_file: Option<String>,

    /// 证书 passphrase（也可通过环境变量 RUP_KEY_PASSPHRASE 设置）
    #[arg(long)]
    key_passphrase: Option<String>,

    /// 使用 SSH agent 认证（Unix only）
    #[arg(long)]
    use_agent: bool,

    /// 要上传的文件名
    #[arg(long)]
    upload_file: Option<String>,

    /// 工作路径
    #[arg(long)]
    work_dir: Option<String>,

    /// 确保工具已安装，不存在时自动安装（可多次指定，如 --ensure unzip --ensure curl）
    #[arg(long)]
    ensure: Vec<String>,

    /// 要执行的命令
    #[arg(long)]
    exec: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), async_ssh2_tokio::Error> {
    let args = Args::parse();

    let user = args.user;
    let host = args.host;
    let port = args.port;

    let password = args.password.or_else(|| std::env::var("RUP_PASSWORD").ok());
    let key_passphrase = args.key_passphrase.or_else(|| std::env::var("RUP_KEY_PASSPHRASE").ok());

    let auth = if args.use_agent {
        AuthMethod::with_agent()
    } else if let Some(key) = args.key_file {
        AuthMethod::with_key_file(key, key_passphrase.as_deref())
    } else if let Some(pwd) = password {
        AuthMethod::with_password(&pwd)
    } else {
        panic!("缺少认证参数：--use-agent | --key-file | --password（或环境变量 RUP_PASSWORD）");
    };

    let client = Client::connect((host.clone(), port), &user, auth, ServerCheckMethod::NoCheck).await?;

    println!("连接成功: {user}@{host}:{port}");

    for tool in &args.ensure {
        let check = client.execute(&format!("command -v {} >/dev/null 2>&1", shell_escape(tool))).await?;
        if check.exit_status != 0 {
            println!("{tool} 未安装，正在安装...");
            let install = client.execute(&format!(
                "apt-get install -y {0} 2>/dev/null || yum install -y {0} 2>/dev/null || apk add {0} 2>/dev/null",
                shell_escape(tool)
            )).await?;
            if install.exit_status != 0 {
                panic!("安装 {tool} 失败，请手动安装后重试");
            }
            println!("{tool} 安装成功");
        }
    }

    if let Some(dir) = args.work_dir.clone() {
        client.execute(&format!("mkdir -p {}", shell_escape(&dir))).await?;
    }

    if let Some(filename) = args.upload_file {
        let dest_full_path = if let Some(dir) = args.work_dir.clone() {
            format!("{dir}/{filename}")
        } else {
            filename.clone()
        };

        let file_size = std::fs::metadata(&filename).map(|m| m.len()).unwrap_or(0);
        let pb = ProgressBar::new(file_size);
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({percent}%) {bytes_per_sec}",
            )
            .unwrap()
            .progress_chars("=>-"),
        );
        pb.enable_steady_tick(Duration::from_millis(100));

        // 通过轮询远程文件大小获取真实进度，与上传并发执行
        let stat_cmd = format!(
            "stat -c %s {0} 2>/dev/null || stat -f %z {0} 2>/dev/null || echo 0",
            shell_escape(&dest_full_path)
        );
        let upload_fut = client.upload_file(&filename, &dest_full_path, None, Some(5 * 1024 * 1024), false);
        let poll_fut = async {
            loop {
                tokio::time::sleep(Duration::from_millis(300)).await;
                if let Ok(result) = client.execute(&stat_cmd).await {
                    if let Ok(bytes) = result.stdout.trim().parse::<u64>() {
                        pb.set_position(bytes.min(file_size));
                    }
                }
            }
        };
        tokio::select! {
            result = upload_fut => { result?; }
            _ = poll_fut => {}
        }

        pb.finish_with_message(format!("{dest_full_path} 文件已上传"));
    }

    for arg in args.exec {
        let cmd = if let Some(dir) = args.work_dir.clone() { format!("cd {dir} && {arg}") } else { arg.clone() };
        let result = client.execute(&cmd).await?;
        println!("执行 `{arg}` -> {}", result.stdout);
    }
    println!("已全部执行");
    client.disconnect().await?;
    println!("ok");
    Ok(())
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

