use std::path::Path;

pub const FALLBACK: &str = "Minecraft has crashed suddenly. Review logs manually.";

const LIBRARIES: [(&str, &str); 9] = [
    ("net.fabricmc.fabric.api", "Fabric API"),
    ("dev.architectury", "Architectury API"),
    ("me.shedaniel.clothconfig", "Cloth Config API"),
    ("me.shedaniel.autoconfig", "Cloth Config API"),
    ("dev.isxander.yacl", "YetAnotherConfigLib"),
    ("com.terraformersmc.modmenu", "Mod Menu"),
    ("software.bernie.geckolib", "GeckoLib"),
    ("net.blay09.mods.balm", "Balm"),
    ("fuzs.puzzleslib", "Puzzles Lib"),
];

pub struct Mods(Vec<(String, String)>);

impl Mods {
    pub fn load(dir: &Path) -> Mods {
        let mut list = Vec::new();
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("jar")) {
                list.extend(crate::content::mod_identity(&path));
            }
        }
        Mods(list)
    }

    fn get(&self, id: &str) -> Option<&str> {
        let id = id.to_ascii_lowercase();
        self.0
            .iter()
            .filter(|(m, _)| id == *m || id.strip_prefix(m.as_str()).is_some_and(|rest| rest.starts_with(['-', '_'])))
            .max_by_key(|(m, _)| m.len())
            .map(|(_, name)| name.as_str())
    }

    fn name(&self, id: &str) -> String {
        self.get(id).unwrap_or(id).to_string()
    }

    fn by_class(&self, class: &str) -> Option<&str> {
        class
            .split(['.', '/'])
            .take_while(|s| s.starts_with(|c: char| c.is_ascii_lowercase()))
            .find_map(|segment| self.0.iter().find(|(m, _)| m.len() > 2 && plain(m) == plain(segment)).map(|(_, name)| name.as_str()))
    }

    fn by_frame(&self, line: &str) -> Option<String> {
        let body = line.trim().strip_prefix("at ")?;
        let parts: Vec<&str> = body.split('$').collect();
        if let Some(pair) = parts.windows(2).find(|w| mixin_tag(w[0])) {
            return Some(self.name(pair[1]));
        }
        let code = body.split('(').next()?;
        let mut segments: Vec<&str> = code.split('/').collect();
        let class = segments.pop()?;
        for module in segments {
            if let Some(name) = module.split_once('@').and_then(|(id, _)| self.get(id)) {
                return Some(name.to_string());
            }
        }
        self.by_class(class).map(String::from)
    }
}

pub fn analyze(log: &[String], report: Option<&str>, mods_dir: &Path) -> String {
    explain(log, report, &Mods::load(mods_dir)).unwrap_or_else(|| FALLBACK.to_string())
}

fn explain(log: &[String], report: Option<&str>, mods: &Mods) -> Option<String> {
    let all: Vec<&str> = report.into_iter().flat_map(str::lines).chain(log.iter().map(String::as_str)).filter(|l| !l.contains("/WARN]")).collect();
    let block = exception_block(log, report);
    incompatible(&all)
        .or_else(|| duplicates(&all))
        .or_else(|| dependencies(&all, mods))
        .or_else(|| failures(&all))
        .or_else(|| unreadable(&all))
        .or_else(|| jvm(&all))
        .or_else(|| memory(&all))
        .or_else(|| java_version(&all, mods))
        .or_else(|| graphics(&all))
        .or_else(|| culprit(&block, &all, mods))
}

fn incompatible(all: &[&str]) -> Option<String> {
    let start = all.iter().position(|l| l.contains("A potential solution has been determined") || l.trim() == "More details:")?;
    let (mut fixes, mut details) = (Vec::<String>::new(), Vec::<String>::new());
    let mut in_details = false;
    for line in &all[start..] {
        let text = line.trim();
        if text == "More details:" {
            in_details = true;
            continue;
        }
        if text.is_empty() || text.starts_with("A potential solution") {
            continue;
        }
        let Some(item) = text.strip_prefix("- ") else { break };
        let list = if in_details { &mut details } else { &mut fixes };
        match list.last_mut() {
            Some(last) if line.starts_with("\t\t") => {
                last.push(' ');
                last.push_str(item);
            }
            _ => list.push(item.to_string()),
        }
    }
    let mut out = vec!["Some mods do not work together.".to_string()];
    out.extend(details.iter().take(3).map(|d| tidy(d)));
    out.extend(fixes.first().map(|f| format!("Fix: {}", tidy(f))));
    (out.len() > 1).then(|| out.join("\n"))
}

fn duplicates(all: &[&str]) -> Option<String> {
    let i = all.iter().position(|l| l.to_ascii_lowercase().contains("duplicate mod"))?;
    let files = all[i..].iter().take(4).find(|l| l.contains(".jar")).map(|l| format!("{}\n", clip(l.trim(), 160))).unwrap_or_default();
    Some(format!("The same mod is installed twice.\n{files}Remove one copy from the mods folder."))
}

fn dependencies(all: &[&str], mods: &Mods) -> Option<String> {
    let mut items: Vec<String> = Vec::new();
    for line in all {
        let (Some(id), Some(by)) = (quoted(line, "Mod ID: '"), quoted(line, "Requested by: '")) else { continue };
        let range = quoted(line, "Expected range: '").unwrap_or("");
        let actual = quoted(line, "Actual version: '").unwrap_or("");
        let item = if actual.is_empty() || actual.contains("MISSING") {
            format!("{} needs {id} {range}, which is missing.", mods.name(by))
        } else {
            format!("{} needs {id} {range}, found {actual}.", mods.name(by))
        };
        if !items.contains(&item) {
            items.push(item);
        }
    }
    (!items.is_empty()).then(|| format!("Some mods are missing or the wrong version.\n{}", items.iter().take(3).cloned().collect::<Vec<_>>().join("\n")))
}

fn failures(all: &[&str]) -> Option<String> {
    let mut items: Vec<String> = Vec::new();
    for (i, line) in all.iter().enumerate() {
        let Some(message) = line.trim().strip_prefix("Failure message: ") else { continue };
        let mut item = sentence(message);
        if let Some(next) = all.get(i + 1).map(|l| l.trim()).filter(|l| l.starts_with("Currently,")) {
            item = format!("{item} {}", sentence(next));
        }
        if !items.contains(&item) {
            items.push(item);
        }
    }
    (!items.is_empty()).then(|| format!("A mod failed to load.\n{}", items.iter().take(3).map(|s| clip(s, 160)).collect::<Vec<_>>().join("\n")))
}

fn unreadable(all: &[&str]) -> Option<String> {
    let line = all.iter().find(|l| l.contains("Error analyzing ["))?;
    let path = line.split_once("Error analyzing [")?.1.split_once(']')?.0;
    let file = path.rsplit(['\\', '/']).next()?;
    let reason = line.trim_end().strip_suffix(')').and_then(|l| l.rsplit_once(" (")).map(|(_, r)| format!("{}\n", sentence(r))).unwrap_or_default();
    Some(format!("Minecraft could not read the mod file {file}.\n{reason}Remove it or download it again."))
}

fn jvm(all: &[&str]) -> Option<String> {
    if let Some(option) = all.iter().find_map(|l| quoted(l, "Unrecognized VM option '")) {
        return Some(format!("Java did not start: unknown option '{option}'.\nRemove it from the JVM arguments in the instance settings."));
    }
    let heap = ["Could not reserve enough space", "Invalid maximum heap size", "Invalid initial heap size", "Initial heap size set to a larger value"];
    if all.iter().any(|l| heap.iter().any(|k| l.contains(k))) {
        return Some("Java could not get the memory it was given.\nLower the memory in the instance settings.".into());
    }
    all.iter().any(|l| l.contains("Could not create the Java Virtual Machine")).then(|| "Java did not start.\nCheck the JVM arguments in the instance settings.".into())
}

fn memory(all: &[&str]) -> Option<String> {
    all.iter().any(|l| l.contains("java.lang.OutOfMemoryError")).then(|| "Minecraft ran out of memory.\nGive it more memory in the instance settings.".into())
}

fn java_version(all: &[&str], mods: &Mods) -> Option<String> {
    let line = all.iter().find(|l| l.contains("UnsupportedClassVersionError"))?;
    let number = |key: &str| line.split_once(key).and_then(|(_, r)| r.split(|c: char| !c.is_ascii_digit()).next()?.parse::<u32>().ok()).and_then(|v| v.checked_sub(44));
    let need = number("class file version ")?;
    let who = after(line, "UnsupportedClassVersionError: ").and_then(|c| mods.by_class(c)).unwrap_or("A mod");
    let have = number("up to ").map(|v| format!("Java {v}")).unwrap_or_else(|| "an older Java".into());
    Some(format!("{who} needs Java {need}, but the game runs on {have}.\nPick Java {need} or newer in the instance settings."))
}

fn graphics(all: &[&str]) -> Option<String> {
    for (i, line) in all.iter().enumerate() {
        if !line.contains("Problematic frame:") {
            continue;
        }
        let lib = all.get(i + 1).and_then(|l| l.split_once('[')).map(|(_, r)| r.split(['+', ']']).next().unwrap_or("").to_ascii_lowercase()).unwrap_or_default();
        let vendor = if ["nvoglv", "nvwgf", "libnvidia"].iter().any(|p| lib.starts_with(p)) {
            "NVIDIA"
        } else if ["atio", "atig", "amdvlk", "amdxc"].iter().any(|p| lib.starts_with(p)) {
            "AMD"
        } else if (lib.starts_with("ig") && lib.contains("icd")) || lib.starts_with("igxel") || lib.starts_with("iris_dri") {
            "Intel"
        } else {
            continue;
        };
        return Some(format!("The {vendor} graphics driver crashed.\nUpdate your graphics driver."));
    }
    let opengl = ["does not appear to support OpenGL", "Pixel format not accelerated", "GLFW error 65542", "GLFW error 65543"];
    all.iter().any(|l| opengl.iter().any(|k| l.contains(k))).then(|| "Your graphics driver does not support OpenGL.\nInstall or update your graphics driver.".into())
}

fn culprit(block: &[&str], all: &[&str], mods: &Mods) -> Option<String> {
    let head = *block.first()?;
    let root = short(block.iter().rev().find(|l| l.starts_with("Caused by:")).copied().unwrap_or(head));
    if let Some(id) = block.iter().find_map(|l| quoted(l, "provided by '")) {
        let name = mods.name(id);
        return Some(format!("{name} failed to start.\n{root}\nUpdate or remove {name}."));
    }
    if let Some(id) = block.iter().find_map(|l| mixin_owner(l)) {
        let name = mods.name(id);
        let target = block
            .iter()
            .find_map(|l| after(l, "Mixin transformation of ").or_else(|| after(l, "in the target class ")))
            .and_then(|c| mods.by_class(c))
            .filter(|t| *t != name.as_str());
        return Some(match target {
            Some(other) => format!("{name} does not work with the installed {other}.\nUpdate or remove {name}."),
            None => format!("{name} does not work with this Minecraft version or another mod.\nUpdate or remove {name}."),
        });
    }
    let frames = || block.iter().find_map(|l| mods.by_frame(l));
    let missing = block.iter().find_map(|l| after(l, "NoClassDefFoundError: ").or_else(|| after(l, "ClassNotFoundException: "))).filter(|c| c.contains(['.', '/']));
    if let Some(class) = missing.map(|c| c.replace('/', ".")) {
        let who = frames().unwrap_or_else(|| "A mod".into());
        return Some(match LIBRARIES.iter().find(|(prefix, _)| class.starts_with(prefix)) {
            Some((_, lib)) => format!("{who} needs {lib}, which is not installed.\nInstall {lib}."),
            None => format!("{who} needs {class}, which is missing.\nA required mod is missing or the wrong version."),
        });
    }
    let name = suspected(all).or_else(frames)?;
    Some(format!("{name} caused the crash.\n{root}\nUpdate or remove {name}."))
}

fn suspected(all: &[&str]) -> Option<String> {
    let i = all.iter().position(|l| l.trim_start().starts_with("Suspected Mod"))?;
    let value = all[i].split_once(':').map(|(_, v)| v.trim()).filter(|v| !v.is_empty()).or_else(|| all.get(i + 1).map(|l| l.trim()))?;
    let name = value.split(", Version").next()?.split(" (").next()?.trim();
    (!name.is_empty() && !name.eq_ignore_ascii_case("none") && !name.eq_ignore_ascii_case("unknown")).then(|| name.to_string())
}

fn mixin_owner(line: &str) -> Option<&str> {
    if let Some(id) = after(line, "Mixin apply for mod ") {
        return Some(id);
    }
    let failed = ["FAILED", "MixinApplyError", "Critical injection failure", "InvalidInjectionException", "InvalidMixinException"];
    if !failed.iter().any(|k| line.contains(k)) {
        return None;
    }
    line.split_once("from mod ").and_then(|(_, r)| r.split([']', ' ', ',']).next()).filter(|id| !id.is_empty())
}

fn exception_block<'a>(log: &'a [String], report: Option<&'a str>) -> Vec<&'a str> {
    if let Some(lines) = report.map(|r| r.lines().collect::<Vec<_>>()) {
        if let Some(i) = lines.iter().position(|l| l.starts_with("Description:")) {
            return lines[i + 1..].iter().copied().skip_while(|l| l.trim().is_empty()).take_while(|l| !l.starts_with("A detailed walkthrough") && !l.starts_with("-- ")).collect();
        }
    }
    let Some(start) = log.iter().rposition(|l| exception_start(l)) else { return Vec::new() };
    std::iter::once(log[start].as_str())
        .chain(log[start + 1..].iter().map(String::as_str).take_while(|l| l.starts_with(char::is_whitespace) || l.starts_with("Caused by:")))
        .collect()
}

fn exception_start(line: &str) -> bool {
    if line.starts_with(char::is_whitespace) || line.starts_with("Caused by") || line.contains("/WARN]") {
        return false;
    }
    let text = line.strip_prefix("Exception in thread ").and_then(|r| r.split_once("\" ")).map(|(_, t)| t).unwrap_or(line);
    let token = text.split([':', ' ']).next().unwrap_or("");
    token.contains('.') && token.starts_with(|c: char| c.is_ascii_lowercase()) && ["Exception", "Error", "Throwable"].iter().any(|s| token.ends_with(s))
}

fn short(line: &str) -> String {
    let mut text = line.trim();
    text = text.strip_prefix("Caused by: ").unwrap_or(text);
    if let Some(rest) = text.strip_prefix("Exception in thread ") {
        text = rest.split_once("\" ").map(|(_, t)| t).unwrap_or(rest);
    }
    let (class, message) = text.split_once(": ").unwrap_or((text, ""));
    let class = class.rsplit('.').next().unwrap_or(class);
    let mut message = message.split(' ').map(simplify).collect::<Vec<_>>().join(" ");
    if message.contains(" are in unnamed module") || message.contains(" are in module") {
        if let Some(i) = message.find(" (") {
            message.truncate(i);
        }
    }
    clip(&if message.is_empty() { class.to_string() } else { format!("{class}: {message}") }, 160)
}

fn simplify(word: &str) -> String {
    if word.contains(":\\") || word.contains(":/") || (word.starts_with('/') && word[1..].contains('/')) {
        return word.rsplit(['\\', '/']).next().unwrap_or(word).to_string();
    }
    let core = word.trim_end_matches([',', ')', ':', ';', '.']);
    let parts: Vec<&str> = core.split('.').collect();
    let last = parts[parts.len() - 1];
    let qualified = parts.len() > 2 && parts[..parts.len() - 1].iter().all(|p| p.starts_with(|c: char| c.is_ascii_lowercase())) && last.starts_with(|c: char| c.is_ascii_uppercase());
    if qualified {
        format!("{last}{}", &word[core.len()..])
    } else {
        word.to_string()
    }
}

fn tidy(item: &str) -> String {
    let lower = item.to_ascii_lowercase();
    let tidied = lower.find("mod '").and_then(|i| {
        let (name, after) = item[i + 5..].split_once("' (")?;
        let (_, after_id) = after.split_once(") ")?;
        let tail = after_id.split_once(' ').map(|(_, t)| t).unwrap_or("");
        Some(format!("{}{name} {tail}", &item[..i]))
    });
    sentence(&tidied.unwrap_or_else(|| item.to_string()))
}

fn sentence(text: &str) -> String {
    let text = text.trim().trim_end_matches('!');
    if text.ends_with(['.', ':']) {
        text.to_string()
    } else {
        format!("{text}.")
    }
}

fn quoted<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.split_once(key).and_then(|(_, r)| r.split_once('\'')).map(|(v, _)| v)
}

fn after<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.split_once(key).and_then(|(_, r)| r.split_whitespace().next()).map(|w| w.trim_end_matches(['.', ',', ':'])).filter(|w| !w.is_empty())
}

fn mixin_tag(s: &str) -> bool {
    s.len() == 6 && s.is_ascii() && s[..3].bytes().all(|b| b.is_ascii_lowercase()) && s[3..].bytes().all(|b| b.is_ascii_digit())
}

fn plain(s: &str) -> String {
    s.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_ascii_lowercase()
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(max - 1).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(list: &[(&str, &str)]) -> Mods {
        Mods(list.iter().map(|(id, name)| (id.to_string(), name.to_string())).collect())
    }

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(String::from).collect()
    }

    #[test]
    fn fabric_dependency_problems() {
        let missing = lines("[main/ERROR]: Incompatible mods found!\nnet.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible with the game or each other!\nA potential solution has been determined, this may resolve your problem:\n\t - Install yet_another_config_lib_v3, any version.\nMore details:\n\t - Mod 'Zoomify' (zoomify) 2.17.0+mc26.2 requires any version of yet_another_config_lib_v3, which is missing!\n\tat net.fabricmc.loader.impl.FormattedException.ofLocalized(FormattedException.java:51)");
        assert_eq!(
            explain(&missing, None, &mods(&[])).unwrap(),
            "Some mods do not work together.\nZoomify requires any version of yet_another_config_lib_v3, which is missing.\nFix: Install yet_another_config_lib_v3, any version."
        );
        let version = lines("A potential solution has been determined, this may resolve your problem:\n\t - Replace mod 'tru3club recovered client' (memory_recovered) 0.2.4-local-configs with any version that is compatible with:\n\t\t - minecraft 26.3\nMore details:\n\t - Mod 'tru3club recovered client' (memory_recovered) 0.2.4-local-configs requires version 26.2 of 'Minecraft' (minecraft), but only the wrong version is present: 26.3!\n\tat net.fabricmc.loader.impl.FabricLoaderImpl.load(FabricLoaderImpl.java:207)");
        assert_eq!(
            explain(&version, None, &mods(&[])).unwrap(),
            "Some mods do not work together.\ntru3club recovered client requires version 26.2 of 'Minecraft' (minecraft), but only the wrong version is present: 26.3.\nFix: Replace tru3club recovered client with any version that is compatible with: minecraft 26.3."
        );
    }

    #[test]
    fn names_the_mod_from_crash_reports() {
        let entrypoint = "---- Minecraft Crash Report ----\nDescription: Initializing game\n\njava.lang.RuntimeException: Could not execute entrypoint stage 'client' due to errors, provided by 'caxton' at 'xyz.flirora.caxton.fabric.CaxtonModGlyphContainerFlavor'!\n\tat net.fabricmc.loader.impl.FabricLoaderImpl.lambda$invokeEntrypoints$0(FabricLoaderImpl.java:418)\n\tSuppressed: java.lang.RuntimeException: Mixin transformation of dev.tr7zw.entityculling.CullTask failed\n\tCaused by: org.spongepowered.asm.mixin.throwables.MixinApplyError: Mixin [fwa.client.mixins.json:compat.entityculling.CullTaskMixin from mod fwa] from phase [DEFAULT] in config [fwa.client.mixins.json] FAILED during APPLY\nCaused by: java.lang.UnsatisfiedLinkError: C:\\Users\\x\\AppData\\Roaming\\Zip\\instances\\fabric-26-3\\minecraft\\caxton_impl.dll: An Application Control policy has blocked this file\n\tat java.base/jdk.internal.loader.NativeLibraries.load(Native Method)\n\nA detailed walkthrough of the error, its code path and all known details is as follows:\n-- Head --";
        assert_eq!(
            explain(&[], Some(entrypoint), &mods(&[("caxton", "Caxton")])).unwrap(),
            "Caxton failed to start.\nUnsatisfiedLinkError: caxton_impl.dll: An Application Control policy has blocked this file\nUpdate or remove Caxton."
        );

        let mixin = "Description: Initializing game\n\njava.lang.RuntimeException: Mixin transformation of dev.tr7zw.entityculling.CullTask failed\n\tat net.fabricmc.loader.impl.launch.knot.KnotClassDelegate.getPostMixinClassByteArray(KnotClassDelegate.java:441)\nCaused by: org.spongepowered.asm.mixin.throwables.MixinApplyError: Mixin [fwa.client.mixins.json:compat.entityculling.CullTaskMixin from mod fwa] from phase [DEFAULT] in config [fwa.client.mixins.json] FAILED during APPLY";
        assert_eq!(
            explain(&[], Some(mixin), &mods(&[("fwa", "FWA"), ("entityculling", "EntityCulling")])).unwrap(),
            "FWA does not work with the installed EntityCulling.\nUpdate or remove FWA."
        );

        let injected = "Description: Initializing game\n\njava.lang.ClassCastException: class net.minecraft.client.renderer.GameRenderer cannot be cast to class me.thosea.badoptimizations.mixin.accessors.GameRendererAccessor (net.minecraft.client.renderer.GameRenderer and me.thosea.badoptimizations.mixin.accessors.GameRendererAccessor are in unnamed module of loader 'knot' @43ee72e6)\n\tat knot//net.minecraft.client.renderer.LightmapRenderStateExtractor.handler$zbd000$badoptimizations$onInit(LightmapRenderStateExtractor.java:542)\n\tat knot//net.minecraft.client.Minecraft.<init>(Minecraft.java)";
        assert_eq!(
            explain(&[], Some(injected), &mods(&[("badoptimizations", "BadOptimizations")])).unwrap(),
            "BadOptimizations caused the crash.\nClassCastException: class GameRenderer cannot be cast to class GameRendererAccessor\nUpdate or remove BadOptimizations."
        );

        let submodule = "Description: Initializing game\n\njava.lang.ClassCastException: class net.minecraft.client.OptionInstance cannot be cast to class com.ishland.c2me.client.uncapvd.mixin.ISimpleOption\n\tat knot//net.minecraft.client.Options.handler$zdl001$c2me-client-uncapvd$onInit(Options.java:4865)";
        assert!(explain(&[], Some(submodule), &mods(&[("c2me", "C2ME")])).unwrap().starts_with("C2ME caused the crash."));
    }

    #[test]
    fn names_the_mod_from_the_log_stack() {
        let log = lines("[Render thread/INFO]: Opening export window\njava.lang.UnsatisfiedLinkError: no jniavutil in java.library.path: C:\\Zip\\natives/java\n\tat java.base/java.lang.ClassLoader.loadLibrary(ClassLoader.java:2285)\n\tat knot//org.bytedeco.javacpp.Loader.loadLibrary(Loader.java:1832)\n\tat knot//com.moulberry.flashback.combo_options.VideoContainer.getSupportedVideoCodecs(VideoContainer.java:65)");
        assert!(explain(&log, None, &mods(&[("flashback", "Flashback")])).unwrap().starts_with("Flashback caused the crash.\nUnsatisfiedLinkError: no jniavutil"));

        let library = lines("Exception in thread \"main\" java.lang.NoClassDefFoundError: dev/architectury/event/events/client/ClientLifecycleEvent\n\tat knot//com.example.coolmod.Client.init(Client.java:5)");
        assert_eq!(explain(&library, None, &mods(&[("coolmod", "Cool Mod")])).unwrap(), "Cool Mod needs Architectury API, which is not installed.\nInstall Architectury API.");
    }

    #[test]
    fn known_problems() {
        let m = mods(&[("roughlyenoughitems", "Roughly Enough Items"), ("coolmod", "Cool Mod")]);
        let check = |text: &str| explain(&lines(text), None, &m).unwrap();
        assert_eq!(
            check("Caused by: java.lang.RuntimeException: Error analyzing [C:\\Zip\\mods\\Bad.jar]: java.io.FileNotFoundException: C:\\Zip\\mods\\Bad.jar (Operation did not complete successfully because the file contains a virus or potentially unwanted software)"),
            "Minecraft could not read the mod file Bad.jar.\nOperation did not complete successfully because the file contains a virus or potentially unwanted software.\nRemove it or download it again."
        );
        assert_eq!(
            check("Missing or unsupported mandatory dependencies:\n\tMod ID: 'architectury', Requested by: 'roughlyenoughitems', Expected range: '[9.1.12,)', Actual version: '[MISSING]'"),
            "Some mods are missing or the wrong version.\nRoughly Enough Items needs architectury [9.1.12,), which is missing."
        );
        assert_eq!(
            check("Unrecognized VM option 'Foo'\nError: Could not create the Java Virtual Machine.\nError: A fatal exception has occurred. Program will exit."),
            "Java did not start: unknown option 'Foo'.\nRemove it from the JVM arguments in the instance settings."
        );
        assert_eq!(
            check("Exception in thread \"main\" java.lang.UnsupportedClassVersionError: com/example/coolmod/Main has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0"),
            "Cool Mod needs Java 21, but the game runs on Java 17.\nPick Java 21 or newer in the instance settings."
        );
        assert_eq!(check("# Problematic frame:\n# C  [atio6axx.dll+0x1a2b3c]"), "The AMD graphics driver crashed.\nUpdate your graphics driver.");
        assert_eq!(check("java.lang.OutOfMemoryError: Java heap space"), "Minecraft ran out of memory.\nGive it more memory in the instance settings.");
    }

    #[test]
    fn falls_back_when_nothing_is_found() {
        let noise = lines("[main/WARN]: Error loading class: mezz/jei/fabric/platform/RenderHelper (java.lang.ClassNotFoundException: mezz/jei/fabric/platform/RenderHelper)\n[main/WARN]: @Mixin target mezz.jei.fabric.platform.RenderHelper was not found appleskin.jei.mixins.json:JEIRenderHelperMixin from mod appleskin\n[Render thread/INFO]: Stopping!");
        assert_eq!(analyze(&noise, None, Path::new("missing-mods-dir")), FALLBACK);
        let vanilla = lines("java.lang.IllegalStateException: Rendersystem called from wrong thread\n\tat knot//net.minecraft.client.Minecraft.run(Minecraft.java:900)");
        assert_eq!(explain(&vanilla, None, &mods(&[("sodium", "Sodium")])), None);
    }
}
