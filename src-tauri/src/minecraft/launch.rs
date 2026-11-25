use super::version::{rules_allow, ArgValue, Features, VersionJson};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct LaunchContext {
    pub version: VersionJson,
    pub game_dir: PathBuf,
    pub natives_dir: PathBuf,
    pub classpath: Vec<PathBuf>,
    pub libraries_dir: PathBuf,
    pub assets_root: PathBuf,
    pub virtual_assets: Option<PathBuf>,
    pub player_name: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: String,
    pub memory_min: u32,
    pub memory_max: u32,
    pub width: u32,
    pub height: u32,
    pub extra_jvm: Vec<String>,
    pub log_config: Option<(String, PathBuf)>,
    pub launcher_version: String,
}

pub fn split_args(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for c in text.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => current.push(c),
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c.is_whitespace() => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            None => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn substitute(value: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some(end) = after.find('}') {
            let key = &after[..end];
            match vars.get(key) {
                Some(v) => out.push_str(v),
                None => {
                    out.push_str("${");
                    out.push_str(key);
                    out.push('}');
                }
            }
            rest = &after[end + 1..];
        } else {
            out.push_str(&rest[start..]);
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

fn collect(values: &[ArgValue], features: &Features, vars: &HashMap<&str, String>, out: &mut Vec<String>) {
    for value in values {
        match value {
            ArgValue::Plain(s) => out.push(substitute(s, vars)),
            ArgValue::Conditional { rules, value } => {
                if rules_allow(&Some(rules.clone()), features) {
                    for item in value.items() {
                        out.push(substitute(&item, vars));
                    }
                }
            }
        }
    }
}

pub fn build(ctx: &LaunchContext) -> Vec<String> {
    let features = Features { custom_resolution: ctx.width > 0 && ctx.height > 0 };
    let path = |p: &PathBuf| p.to_string_lossy().to_string();
    let classpath = ctx.classpath.iter().map(path).collect::<Vec<_>>().join(crate::platform::CLASSPATH_SEPARATOR);
    let assets_index = ctx
        .version
        .asset_index
        .as_ref()
        .map(|a| a.id.clone())
        .or_else(|| ctx.version.assets.clone())
        .unwrap_or_else(|| "legacy".into());
    let game_assets = ctx.virtual_assets.clone().unwrap_or_else(|| ctx.assets_root.clone());
    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", ctx.player_name.clone());
    vars.insert("version_name", ctx.version.id.clone());
    vars.insert("game_directory", path(&ctx.game_dir));
    vars.insert("assets_root", path(&ctx.assets_root));
    vars.insert("game_assets", path(&game_assets));
    vars.insert("assets_index_name", assets_index);
    vars.insert("auth_uuid", ctx.uuid.clone());
    vars.insert("auth_access_token", ctx.access_token.clone());
    vars.insert("auth_session", if ctx.user_type == "msa" { format!("token:{}:{}", ctx.access_token, ctx.uuid) } else { "-".into() });
    vars.insert("auth_xuid", "0".into());
    vars.insert("clientid", "0".into());
    vars.insert("user_type", ctx.user_type.clone());
    vars.insert("user_properties", "{}".into());
    vars.insert("version_type", ctx.version.kind.clone().unwrap_or_else(|| "release".into()));
    vars.insert("natives_directory", path(&ctx.natives_dir));
    vars.insert("launcher_name", "zip".into());
    vars.insert("launcher_version", ctx.launcher_version.clone());
    vars.insert("classpath", classpath);
    vars.insert("classpath_separator", crate::platform::CLASSPATH_SEPARATOR.into());
    vars.insert("library_directory", path(&ctx.libraries_dir));
    vars.insert("resolution_width", ctx.width.to_string());
    vars.insert("resolution_height", ctx.height.to_string());

    let mut args = Vec::new();
    if ctx.memory_min > 0 {
        args.push(format!("-Xms{}M", ctx.memory_min));
    }
    args.push(format!("-Xmx{}M", ctx.memory_max.max(512)));
    match ctx.version.arguments.as_ref().filter(|a| !a.jvm.is_empty()) {
        Some(arguments) => collect(&arguments.jvm, &features, &vars, &mut args),
        None => {
            args.push("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".into());
            args.push(substitute("-Djava.library.path=${natives_directory}", &vars));
            args.push("-cp".into());
            args.push(vars["classpath"].clone());
        }
    }
    let main_class = ctx.version.main_class.clone().unwrap_or_else(|| "net.minecraft.client.main.Main".into());
    if main_class.contains("launchwrapper") {
        args.push("-Dfml.ignoreInvalidMinecraftCertificates=true".into());
        args.push("-Dfml.ignorePatchDiscrepancies=true".into());
    }
    if let Some((argument, file)) = &ctx.log_config {
        args.push(argument.replace("${path}", &path(file)));
    }
    args.extend(ctx.extra_jvm.iter().cloned());
    args.push(main_class);
    match (&ctx.version.arguments, &ctx.version.minecraft_arguments) {
        (_, Some(legacy)) => {
            for token in split_args(legacy) {
                args.push(substitute(&token, &vars));
            }
            if features.custom_resolution {
                args.extend(["--width".into(), ctx.width.to_string(), "--height".into(), ctx.height.to_string()]);
            }
        }
        (Some(arguments), None) => collect(&arguments.game, &features, &vars, &mut args),
        (None, None) => {}
    }
    args
}

pub fn redact(args: &[String], token: &str) -> String {
    args.iter()
        .map(|a| if !token.is_empty() && token != "0" && a.contains(token) { a.replace(token, "<redacted>") } else { a.clone() })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(version: VersionJson) -> LaunchContext {
        LaunchContext {
            version,
            game_dir: PathBuf::from("G"),
            natives_dir: PathBuf::from("N"),
            classpath: vec![PathBuf::from("a.jar"), PathBuf::from("b.jar")],
            libraries_dir: PathBuf::from("L"),
            assets_root: PathBuf::from("A"),
            virtual_assets: None,
            player_name: "Steve".into(),
            uuid: "uuid".into(),
            access_token: "secret-token".into(),
            user_type: "msa".into(),
            memory_min: 512,
            memory_max: 2048,
            width: 0,
            height: 0,
            extra_jvm: vec!["-Dextra=1".into()],
            log_config: Some(("-Dlog4j.configurationFile=${path}".into(), PathBuf::from("log.xml"))),
            launcher_version: "1.0.0".into(),
        }
    }

    #[test]
    fn modern_arguments() {
        let v: VersionJson = serde_json::from_str(r#"{"id":"1.21.1","type":"release","mainClass":"net.minecraft.client.main.Main","assetIndex":{"id":"17","url":"u"},"arguments":{"game":["--username","${auth_player_name}","--assetIndex","${assets_index_name}",{"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],"value":["--width","${resolution_width}"]},{"rules":[{"action":"allow","features":{"is_demo_user":true}}],"value":"--demo"}],"jvm":[{"rules":[{"action":"allow","os":{"name":"osx"}}],"value":["-XstartOnFirstThread"]},"-Djava.library.path=${natives_directory}","-cp","${classpath}"]}}"#).unwrap();
        let args = build(&ctx(v.clone()));
        assert_eq!(args[0], "-Xms512M");
        assert_eq!(args[1], "-Xmx2048M");
        assert!(args.contains(&"-Djava.library.path=N".to_string()));
        assert!(args.contains(&format!("a.jar{}b.jar", crate::platform::CLASSPATH_SEPARATOR)));
        assert!(!args.iter().any(|a| a == "-XstartOnFirstThread"));
        assert!(!args.iter().any(|a| a == "--demo" || a == "--width"));
        assert!(args.contains(&"-Dlog4j.configurationFile=log.xml".to_string()));
        let main = args.iter().position(|a| a == "net.minecraft.client.main.Main").unwrap();
        assert_eq!(args[main + 2], "Steve");
        assert_eq!(args[main + 4], "17");
        let mut c = ctx(v);
        c.width = 1280;
        c.height = 720;
        let args = build(&c);
        assert!(args.iter().any(|a| a == "--width"));
        assert!(args.iter().any(|a| a == "1280"));
    }

    #[test]
    fn legacy_arguments() {
        let v: VersionJson = serde_json::from_str(r#"{"id":"1.7.10","mainClass":"net.minecraft.launchwrapper.Launch","minecraftArguments":"--username ${auth_player_name} --session ${auth_session} --gameDir ${game_directory} --tweakClass cpw.mods.fml.common.launcher.FMLTweaker"}"#).unwrap();
        let mut c = ctx(v);
        c.width = 854;
        c.height = 480;
        let args = build(&c);
        assert!(args.contains(&"-Djava.library.path=N".to_string()));
        assert!(args.contains(&"-Dfml.ignoreInvalidMinecraftCertificates=true".to_string()));
        assert!(args.contains(&"token:secret-token:uuid".to_string()));
        assert!(args.contains(&"cpw.mods.fml.common.launcher.FMLTweaker".to_string()));
        assert_eq!(args[args.len() - 3], "854");
        let shown = redact(&args, "secret-token");
        assert!(!shown.contains("secret-token"));
    }

    #[test]
    fn splits_quoted_args() {
        assert_eq!(split_args("-Xss2M  \"-Dfoo=a b\" -Dx='y z'"), vec!["-Xss2M", "-Dfoo=a b", "-Dx=y z"]);
        assert!(split_args("   ").is_empty());
    }
}
