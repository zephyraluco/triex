use crate::search::index::{ItemKind, RawSearchItem};

/// Built-in Windows system commands searchable directly from the launcher.
pub fn builtin_system_commands() -> Vec<RawSearchItem> {
    vec![
        sys_cmd(
            "lock",
            "锁定屏幕",
            "锁定当前 Windows 会话",
            "sys:lock",
            &["lock", "suoping", "screen"],
        ),
        sys_cmd(
            "sleep",
            "睡眠",
            "使电脑进入睡眠模式",
            "sys:sleep",
            &["sleep", "shuimian", "suspend"],
        ),
        sys_cmd(
            "restart",
            "重启电脑",
            "立即重新启动 Windows",
            "sys:restart",
            &["restart", "reboot", "chongqi"],
        ),
        sys_cmd(
            "shutdown",
            "关机",
            "立即关闭电脑",
            "sys:shutdown",
            &["shutdown", "poweroff", "guanji"],
        ),
        sys_cmd(
            "taskmgr",
            "任务管理器",
            "打开 Windows 任务管理器",
            "sys:taskmgr",
            &["taskmgr", "task manager", "renwu"],
        ),
        sys_cmd(
            "control",
            "控制面板",
            "打开经典控制面板",
            "sys:control",
            &["control", "panel", "kongzhimianban"],
        ),
        sys_cmd(
            "env",
            "环境变量",
            "编辑系统环境变量",
            "sys:env",
            &["env", "path", "environment", "huanjingbianliang"],
        ),
        sys_cmd(
            "devmgmt",
            "设备管理器",
            "打开设备管理器",
            "sys:devmgmt",
            &["devmgmt", "device manager", "shebei"],
        ),
        sys_cmd(
            "settings",
            "Windows 设置",
            "打开系统设置面板",
            "sys:settings",
            &["settings", "shezhi", "ms-settings"],
        ),
        sys_cmd(
            "explorer",
            "文件资源管理器",
            "打开文件资源管理器",
            "sys:explorer",
            &["explorer", "files", "wenjian"],
        ),
        sys_cmd(
            "recycle-bin",
            "回收站",
            "打开桌面回收站",
            "sys:recycle-bin",
            &["recycle", "trash", "huishouzhan"],
        ),
        sys_cmd(
            "triex-settings",
            "Triex 偏好设置",
            "打开启动器设置与插件管理",
            "triex:settings",
            &["triex", "preferences", "config", "shezhi"],
        ),
    ]
}

fn sys_cmd(
    slug: &str,
    name: &str,
    subtitle: &str,
    exec_path: &str,
    keywords: &[&str],
) -> RawSearchItem {
    RawSearchItem {
        id: format!("sys:{slug}"),
        kind: ItemKind::SystemCmd,
        name: name.to_string(),
        subtitle: subtitle.to_string(),
        exec_path: exec_path.to_string(),
        exec_args: String::new(),
        working_dir: String::new(),
        icon_key: String::new(),
        keywords: keywords.iter().map(|k| (*k).to_string()).collect(),
    }
}
