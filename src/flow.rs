use {
    crate::changelog::{self, Changelog, Release, Section},
    bearask::{AskOption, Confirm, Select, TextInput},
    chrono::Local,
    miette::Result,
};

pub fn run(path: &str) -> Result<()> {
    print_banner();

    if !changelog::exists(path) {
        prompt_init(path)?;
    }

    loop {
        let cl = changelog::load(path)?;
        print_unreleased_summary(cl.unreleased());

        match prompt_action()? {
            Action::Add => prompt_add(path)?,
            Action::Release => {
                prompt_release(path)?;
                break;
            }
            Action::View => print_full_view(&cl),
            Action::Skip => {
                println!("  Skipping changelog update — commit will proceed.\n");
                break;
            }
        }
    }

    Ok(())
}

#[derive(Clone)]
enum Action {
    Add,
    Release,
    View,
    Skip,
}

fn prompt_action() -> Result<Action> {
    let options = vec![
        AskOption::new(
            "Add an entry",
            "Record a change in the [Unreleased] section",
            Action::Add,
        ),
        AskOption::new(
            "Cut a release",
            "Promote [Unreleased] to a versioned release",
            Action::Release,
        ),
        AskOption::new(
            "View changelog",
            "Pretty-print the full changelog",
            Action::View,
        ),
        AskOption::new(
            "Skip / done",
            "Leave the changelog as-is and proceed with the commit",
            Action::Skip,
        ),
    ];

    let chosen = Select::new("What would you like to do?")
        .with_options(options)
        .with_help_message("up/down navigate · Enter confirm · Esc to skip")
        .ask()?;

    Ok(chosen.value)
}

fn prompt_init(path: &str) -> Result<()> {
    println!("  No {path} found in this directory.\n");

    let create = Confirm::new(format!("Create a new {path}?"))
        .with_default(true)
        .ask()?;

    if !create {
        println!("  Skipping changelog creation — commit will proceed.\n");
        return Ok(());
    }

    let cl = Changelog::new_empty();
    changelog::save(path, &cl)?;
    println!("  Created {path}\n");
    Ok(())
}

fn prompt_add(path: &str) -> Result<()> {
    let section_options: Vec<AskOption<Section>> = Section::all()
        .iter()
        .map(|s| AskOption::new(s.as_str(), s.description(), s.clone()))
        .collect();

    let chosen_section = Select::new("Which section?")
        .with_options(section_options)
        .with_help_message("Pick the type that best describes your change")
        .ask()?;

    let section = chosen_section.value;

    let entry = TextInput::new("Describe the change")
        .with_placeholder("e.g. Add dark-mode toggle to settings page")
        .with_help_message("Keep it short — this becomes a bullet in the changelog")
        .with_validation(|input: &str| {
            let t = input.trim();
            if t.is_empty() {
                bearask::validation!(invalid "Description cannot be empty")
            } else if t.len() > 280 {
                bearask::validation!(invalid format!("Too long ({} chars, max 280)", t.len()))
            } else {
                bearask::validation!(valid)
            }
        })
        .ask()?;

    println!();
    println!("  Preview:");
    println!("  ### {}", section.as_str());
    println!("  - {}", entry.trim());
    println!();

    let ok = Confirm::new("Add this entry?").with_default(true).ask()?;

    if !ok {
        println!("  Discarded.\n");
        return Ok(());
    }

    let mut cl = changelog::load(path)?;
    cl.unreleased_mut()
        .get_section_mut(&section)
        .push(entry.trim().to_string());
    changelog::save(path, &cl)?;

    println!("  Entry saved to [Unreleased].\n");
    Ok(())
}

fn prompt_release(path: &str) -> Result<()> {
    let cl = changelog::load(path)?;

    if cl.unreleased().map(|r| r.is_empty()).unwrap_or(true) {
        println!("  [Unreleased] is empty — nothing to release.");
        println!("  Add some entries first, then cut a release.\n");
        return Ok(());
    }

    let suggested = cl
        .latest_version()
        .and_then(bump_patch)
        .unwrap_or_else(|| "0.1.0".to_string());

    let version = TextInput::new("Version number")
        .with_default(&suggested)
        .with_placeholder("e.g. 1.2.3")
        .with_help_message("Semantic Versioning: MAJOR.MINOR.PATCH")
        .with_validation(|input: &str| {
            if semver_like(input) {
                bearask::validation!(valid)
            } else {
                bearask::validation!(invalid "Must look like 1.2.3")
            }
        })
        .ask()?;

    let today = Local::now().format("%Y-%m-%d").to_string();
    let date = TextInput::new("Release date")
        .with_default(&today)
        .with_help_message("ISO 8601 — YYYY-MM-DD")
        .with_validation(|input: &str| {
            let parts: Vec<&str> = input.split('-').collect();
            if parts.len() == 3
                && parts[0].len() == 4
                && parts[1].len() == 2
                && parts[2].len() == 2
                && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
            {
                bearask::validation!(valid)
            } else {
                bearask::validation!(invalid "Must be YYYY-MM-DD")
            }
        })
        .ask()?;

    println!();
    println!("  [Unreleased] -> [{version}] - {date}");
    println!("  A new empty [Unreleased] block will be prepended.");
    println!();

    let ok = Confirm::new("Proceed with the release?")
        .with_default(true)
        .ask()?;

    if !ok {
        println!("  Aborted — nothing changed.\n");
        return Ok(());
    }

    let mut cl = changelog::load(path)?;
    let unreleased = cl.unreleased_mut();
    unreleased.version = Some(version.trim().to_string());
    unreleased.date = Some(date.trim().to_string());
    cl.releases.insert(0, Release::unreleased());
    changelog::save(path, &cl)?;

    println!("  Released [{version}] - {date}.\n");
    Ok(())
}

fn print_full_view(cl: &Changelog) {
    println!();
    for release in &cl.releases {
        let heading = match &release.version {
            None => "\x1b[1;36m[Unreleased]\x1b[0m".to_string(),
            Some(v) => {
                let date = release
                    .date
                    .as_deref()
                    .map(|d| format!(" — {d}"))
                    .unwrap_or_default();
                let yanked = if release.yanked {
                    "  \x1b[31m[YANKED]\x1b[0m"
                } else {
                    ""
                };
                format!("\x1b[1;34m[{v}]\x1b[0m{date}{yanked}")
            }
        };
        println!("  {heading}");

        if release.is_empty() {
            println!("    (no entries)");
        } else {
            for (section, items) in &release.entries {
                if items.is_empty() {
                    continue;
                }
                println!("    \x1b[1;33m{}\x1b[0m", section.as_str());
                for item in items {
                    println!("      • {item}");
                }
            }
        }
        println!();
    }
}

fn print_unreleased_summary(unreleased: Option<&Release>) {
    println!("  \x1b[1;36m[Unreleased]\x1b[0m");
    match unreleased {
        Some(_) if unreleased.map(|r| r.is_empty()).unwrap_or(true) => {
            println!("    (empty)\n");
        }
        None => println!("    (empty)\n"),
        Some(r) => {
            for (section, items) in &r.entries {
                if items.is_empty() {
                    continue;
                }
                println!("    {} ({})", section.as_str(), items.len());
                for item in items.iter().take(3) {
                    println!("      • {item}");
                }
                if items.len() > 3 {
                    println!("      ... and {} more", items.len() - 3);
                }
            }
            println!();
        }
    }
}

fn print_banner() {
    println!();
    println!("  \x1b[1mChangelog Assistant\x1b[0m");
    println!("  \x1b[2mKeep A Changelog 1.1.0 · git pre-commit hook\x1b[0m");
    println!();
}

fn semver_like(s: &str) -> bool {
    let parts: Vec<&str> = s.trim().split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok())
}

fn bump_patch(v: &str) -> Option<String> {
    let p: Vec<&str> = v.split('.').collect();
    if p.len() != 3 {
        return None;
    }
    let patch: u32 = p[2].parse().ok()?;
    Some(format!("{}.{}.{}", p[0], p[1], patch + 1))
}
