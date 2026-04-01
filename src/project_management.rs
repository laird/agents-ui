use std::collections::BTreeMap;

/// Supported project-management backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectManagementBackend {
    #[default]
    GitHub,
    Linear,
    Jira,
}

/// Backend-specific knobs kept behind the wrapper boundary.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackendOptions {
    /// Optional backend auth profile/alias.
    pub auth_profile: Option<String>,
    /// Mapping from canonical field names to backend-specific names.
    ///
    /// Canonical keys currently used by the wrapper:
    /// - `title`
    /// - `body`
    /// - `labels`
    pub field_mapping: BTreeMap<String, String>,
}

/// A concrete CLI command to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectCommand {
    pub program: &'static str,
    pub args: Vec<String>,
}

impl ProjectCommand {
    fn new(program: &'static str, args: Vec<String>) -> Self {
        Self { program, args }
    }
}

/// Wrapper for project-management operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectManagementClient {
    backend: ProjectManagementBackend,
    options: BackendOptions,
}

impl Default for ProjectManagementClient {
    fn default() -> Self {
        Self::github()
    }
}

impl ProjectManagementClient {
    pub fn github() -> Self {
        Self {
            backend: ProjectManagementBackend::GitHub,
            options: BackendOptions::default(),
        }
    }

    /// Create a client from runtime environment.
    ///
    /// - `AGENTS_UI_PM_BACKEND`: `github` (default), `linear`, or `jira`
    /// - `AGENTS_UI_PM_AUTH_PROFILE`: optional backend auth profile
    pub fn from_env() -> Self {
        let backend = match std::env::var("AGENTS_UI_PM_BACKEND")
            .ok()
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "linear" => ProjectManagementBackend::Linear,
            "jira" => ProjectManagementBackend::Jira,
            _ => ProjectManagementBackend::GitHub,
        };
        let auth_profile = std::env::var("AGENTS_UI_PM_AUTH_PROFILE")
            .ok()
            .filter(|s| !s.is_empty());
        Self::new(
            backend,
            BackendOptions {
                auth_profile,
                field_mapping: BTreeMap::new(),
            },
        )
    }

    pub fn new(backend: ProjectManagementBackend, options: BackendOptions) -> Self {
        Self { backend, options }
    }

    pub fn list_open_issues(&self, json_fields: &str, limit: usize) -> ProjectCommand {
        let mut args = vec![
            "issue".to_string(),
            "list".to_string(),
            "--state".to_string(),
            "open".to_string(),
            "--limit".to_string(),
            limit.to_string(),
            "--json".to_string(),
            json_fields.to_string(),
        ];
        self.append_backend_auth_args(&mut args);
        ProjectCommand::new(self.program(), args)
    }

    pub fn view_issue(&self, issue_id: impl std::fmt::Display) -> ProjectCommand {
        let mut args = vec![
            "issue".to_string(),
            "view".to_string(),
            issue_id.to_string(),
        ];
        self.append_backend_auth_args(&mut args);
        ProjectCommand::new(self.program(), args)
    }

    pub fn view_issue_web(&self, issue_id: impl std::fmt::Display) -> ProjectCommand {
        let mut args = vec![
            "issue".to_string(),
            "view".to_string(),
            issue_id.to_string(),
            "--web".to_string(),
        ];
        self.append_backend_auth_args(&mut args);
        ProjectCommand::new(self.program(), args)
    }

    pub fn view_issue_json(
        &self,
        issue_id: impl std::fmt::Display,
        fields: &str,
    ) -> ProjectCommand {
        let mut args = vec![
            "issue".to_string(),
            "view".to_string(),
            issue_id.to_string(),
            "--json".to_string(),
            fields.to_string(),
        ];
        self.append_backend_auth_args(&mut args);
        ProjectCommand::new(self.program(), args)
    }

    pub fn create_issue(
        &self,
        title: &str,
        body: Option<&str>,
        labels: &[String],
    ) -> ProjectCommand {
        let title_flag = format!("--{}", self.field_name("title"));
        let body_flag = format!("--{}", self.field_name("body"));
        let labels_flag = format!("--{}", self.field_name("labels"));

        let mut args = vec![
            "issue".to_string(),
            "create".to_string(),
            title_flag,
            title.to_string(),
        ];

        if let Some(body) = body {
            args.push(body_flag);
            args.push(body.to_string());
        }

        if !labels.is_empty() {
            args.push(labels_flag);
            args.push(labels.join(","));
        }

        self.append_backend_auth_args(&mut args);
        ProjectCommand::new(self.program(), args)
    }

    pub fn comment_issue(&self, issue_id: impl std::fmt::Display, body: &str) -> ProjectCommand {
        let mut args = vec![
            "issue".to_string(),
            "comment".to_string(),
            issue_id.to_string(),
            "--body".to_string(),
            body.to_string(),
        ];
        self.append_backend_auth_args(&mut args);
        ProjectCommand::new(self.program(), args)
    }

    pub fn remove_label(&self, issue_id: impl std::fmt::Display, label: &str) -> ProjectCommand {
        let mut args = vec![
            "issue".to_string(),
            "edit".to_string(),
            issue_id.to_string(),
            "--remove-label".to_string(),
            label.to_string(),
        ];
        self.append_backend_auth_args(&mut args);
        ProjectCommand::new(self.program(), args)
    }

    /// Command text sent to manager sessions for issue creation.
    pub fn manager_create_issue_prompt(&self, labels_csv: &str, title: &str) -> String {
        let backend = match self.backend {
            ProjectManagementBackend::GitHub => "gh",
            ProjectManagementBackend::Linear => "linear",
            ProjectManagementBackend::Jira => "jira",
        };
        format!("create {backend} issue --label \"{labels_csv}\" \"{title}\"")
    }

    fn program(&self) -> &'static str {
        match self.backend {
            ProjectManagementBackend::GitHub => "gh",
            ProjectManagementBackend::Linear => "linear",
            ProjectManagementBackend::Jira => "jira",
        }
    }

    fn field_name(&self, canonical: &str) -> String {
        if let Some(mapped) = self.options.field_mapping.get(canonical) {
            return mapped.clone();
        }
        match (self.backend, canonical) {
            (ProjectManagementBackend::GitHub, "title") => "title".to_string(),
            (ProjectManagementBackend::GitHub, "body") => "body".to_string(),
            (ProjectManagementBackend::GitHub, "labels") => "label".to_string(),

            (ProjectManagementBackend::Linear, "title") => "title".to_string(),
            (ProjectManagementBackend::Linear, "body") => "description".to_string(),
            (ProjectManagementBackend::Linear, "labels") => "label".to_string(),

            (ProjectManagementBackend::Jira, "title") => "summary".to_string(),
            (ProjectManagementBackend::Jira, "body") => "description".to_string(),
            (ProjectManagementBackend::Jira, "labels") => "labels".to_string(),
            (_, other) => other.to_string(),
        }
    }

    fn append_backend_auth_args(&self, args: &mut Vec<String>) {
        if self.backend == ProjectManagementBackend::GitHub {
            return;
        }
        if let Some(profile) = self.options.auth_profile.as_ref() {
            args.push("--profile".to_string());
            args.push(profile.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BackendOptions, ProjectManagementBackend, ProjectManagementClient};

    #[test]
    fn github_list_open_issues_matches_existing_command_shape() {
        let cmd =
            ProjectManagementClient::github().list_open_issues("number,title,state,labels", 100);
        assert_eq!(cmd.program, "gh");
        assert_eq!(
            cmd.args,
            vec![
                "issue",
                "list",
                "--state",
                "open",
                "--limit",
                "100",
                "--json",
                "number,title,state,labels",
            ]
        );
    }

    #[test]
    fn github_create_issue_matches_existing_flags() {
        let labels = vec!["enhancement".to_string(), "P3".to_string()];
        let cmd =
            ProjectManagementClient::github().create_issue("Test title", Some("Body"), &labels);
        assert_eq!(cmd.program, "gh");
        assert_eq!(
            cmd.args,
            vec![
                "issue",
                "create",
                "--title",
                "Test title",
                "--body",
                "Body",
                "--label",
                "enhancement,P3",
            ]
        );
    }

    #[test]
    fn github_comment_issue_matches_existing_flags() {
        let cmd = ProjectManagementClient::github().comment_issue(235, "Looks good");
        assert_eq!(cmd.program, "gh");
        assert_eq!(
            cmd.args,
            vec!["issue", "comment", "235", "--body", "Looks good"]
        );
    }

    #[test]
    fn linear_support_uses_linear_cli_program() {
        let labels = vec!["enhancement".to_string()];
        let cmd = ProjectManagementClient::new(
            ProjectManagementBackend::Linear,
            BackendOptions::default(),
        )
        .create_issue("Title", Some("Body"), &labels);
        assert_eq!(cmd.program, "linear");
        assert_eq!(
            cmd.args,
            vec![
                "issue",
                "create",
                "--title",
                "Title",
                "--description",
                "Body",
                "--label",
                "enhancement",
            ]
        );
    }

    #[test]
    fn jira_support_accepts_backend_specific_field_mapping() {
        let mut options = BackendOptions::default();
        options
            .field_mapping
            .insert("labels".to_string(), "tag".to_string());
        let labels = vec!["P3".to_string()];
        let cmd = ProjectManagementClient::new(ProjectManagementBackend::Jira, options)
            .create_issue("Title", None, &labels);

        assert_eq!(cmd.program, "jira");
        assert_eq!(
            cmd.args,
            vec!["issue", "create", "--summary", "Title", "--tag", "P3",]
        );
    }
}
