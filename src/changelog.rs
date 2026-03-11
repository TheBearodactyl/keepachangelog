use {
    miette::{IntoDiagnostic, Result},
    std::{fs, path::Path},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Section {
    Added,
    Changed,
    Deprecated,
    Removed,
    Fixed,
    Security,
}

impl Section {
    pub fn all() -> &'static [Section] {
        use Section::*;
        &[Added, Changed, Deprecated, Removed, Fixed, Security]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Section::Added => "Added",
            Section::Changed => "Changed",
            Section::Deprecated => "Deprecated",
            Section::Removed => "Removed",
            Section::Fixed => "Fixed",
            Section::Security => "Security",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Section::Added => "New features",
            Section::Changed => "Changes to existing functionality",
            Section::Deprecated => "Features to be removed in upcoming releases",
            Section::Removed => "Features removed in this release",
            Section::Fixed => "Any bug fixes",
            Section::Security => "Vulnerabilities patched",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Added" => Some(Section::Added),
            "Changed" => Some(Section::Changed),
            "Deprecated" => Some(Section::Deprecated),
            "Removed" => Some(Section::Removed),
            "Fixed" => Some(Section::Fixed),
            "Security" => Some(Section::Security),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Release {
    pub version: Option<String>,
    pub date: Option<String>,
    pub yanked: bool,
    pub entries: Vec<(Section, Vec<String>)>,
}

impl Release {
    pub fn unreleased() -> Self {
        Self {
            version: None,
            date: None,
            yanked: false,
            entries: vec![],
        }
    }

    pub fn get_section_mut(&mut self, section: &Section) -> &mut Vec<String> {
        if let Some(pos) = self.entries.iter().position(|(s, _)| s == section) {
            return &mut self.entries[pos].1;
        }
        self.entries.push((section.clone(), vec![]));
        let last = self.entries.len() - 1;
        &mut self.entries[last].1
    }

    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(|(_, v)| v.is_empty())
    }

    #[allow(dead_code)]
    pub fn all_entries(&self) -> Vec<(&Section, &str)> {
        self.entries
            .iter()
            .flat_map(|(s, items)| items.iter().map(move |i| (s, i.as_str())))
            .collect()
    }
}

#[derive(Debug)]
pub struct Changelog {
    pub preamble: String,
    pub releases: Vec<Release>,
}

impl Changelog {
    pub fn new_empty() -> Self {
        let preamble =
            "# Changelog\n\n\
             All notable changes to this project will be documented in this file.\n\n\
             The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),\n\
             and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).\n"
            .to_string();
        Self {
            preamble,
            releases: vec![Release::unreleased()],
        }
    }

    pub fn unreleased_mut(&mut self) -> &mut Release {
        if self.releases.first().map(|r| r.version.is_none()) != Some(true) {
            self.releases.insert(0, Release::unreleased());
        }
        &mut self.releases[0]
    }

    pub fn unreleased(&self) -> Option<&Release> {
        self.releases.first().filter(|r| r.version.is_none())
    }

    pub fn latest_version(&self) -> Option<&str> {
        self.releases
            .iter()
            .find(|r| r.version.is_some())
            .and_then(|r| r.version.as_deref())
    }

    pub fn parse(src: &str) -> Self {
        let mut preamble_lines: Vec<&str> = vec![];
        let mut releases: Vec<Release> = vec![];
        let mut current: Option<Release> = None;
        let mut cur_section: Option<Section> = None;
        let mut in_preamble = true;

        for line in src.lines() {
            if line.starts_with("## ") {
                in_preamble = false;
                if let Some(r) = current.take() {
                    releases.push(r);
                }
                cur_section = None;
                current = Some(parse_heading(line));
                continue;
            }
            if line.starts_with("### ") {
                cur_section = Section::from_str(line.trim_start_matches("### ").trim());
                continue;
            }
            if let Some(text) = line.strip_prefix("- ") {
                if let (Some(r), Some(s)) = (current.as_mut(), &cur_section) {
                    r.get_section_mut(s).push(text.to_string());
                }
                continue;
            }
            if in_preamble {
                preamble_lines.push(line);
            }
        }
        if let Some(r) = current {
            releases.push(r);
        }

        Self {
            preamble: preamble_lines.join("\n"),
            releases,
        }
    }

    pub fn render(&self) -> String {
        let mut out = self.preamble.clone();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        for r in &self.releases {
            out.push_str(&render_release(r));
        }
        out
    }
}

fn parse_heading(line: &str) -> Release {
    let body = line.trim_start_matches("## ").trim();
    let yanked = body.contains("[YANKED]");
    let body = body.replace("[YANKED]", "");
    let body = body.trim();

    if body.to_lowercase().contains("unreleased") {
        return Release::unreleased();
    }

    let version = body
        .trim_start_matches('[')
        .split(']')
        .next()
        .unwrap_or("")
        .to_string();
    let date = body.split(" - ").nth(1).map(|d| d.trim().to_string());

    Release {
        version: Some(version),
        date,
        yanked,
        entries: vec![],
    }
}

fn render_release(r: &Release) -> String {
    let mut out = String::new();

    let heading = match &r.version {
        None => "## [Unreleased]".to_string(),
        Some(v) => {
            let date = r
                .date
                .as_deref()
                .map(|d| format!(" - {d}"))
                .unwrap_or_default();
            let yanked = if r.yanked { " [YANKED]" } else { "" };
            format!("## [{v}]{date}{yanked}")
        }
    };
    out.push_str(&heading);
    out.push('\n');

    for section in Section::all() {
        if let Some((_, items)) = r.entries.iter().find(|(s, _)| s == section)
            && !items.is_empty()
        {
            out.push_str(&format!("\n### {}\n\n", section.as_str()));
            for item in items {
                out.push_str(&format!("- {item}\n"));
            }
        }
    }

    out.push('\n');
    out
}

pub fn load(path: &str) -> Result<Changelog> {
    let src = fs::read_to_string(path).into_diagnostic()?;
    Ok(Changelog::parse(&src))
}

pub fn save(path: &str, cl: &Changelog) -> Result<()> {
    fs::write(path, cl.render()).into_diagnostic()
}

pub fn exists(path: &str) -> bool {
    Path::new(path).exists()
}
