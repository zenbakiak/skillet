use serde_json;

pub fn extract_auth_header(request: &str) -> Option<String> {
    // Look for Authorization header in request
    for line in request.lines() {
        let line = line.trim();
        if line.to_lowercase().starts_with("authorization:") {
            let auth_value = line[14..].trim(); // Skip "authorization:"
            if auth_value.to_lowercase().starts_with("bearer ") {
                return Some(auth_value[7..].trim().to_string()); // Skip "bearer "
            }
            // Also support direct token without "Bearer" prefix
            return Some(auth_value.to_string());
        }
    }
    None
}

pub fn check_authentication(request: &str, server_token: &Option<String>) -> Option<String> {
    if let Some(cfg_token) = server_token {
        let auth_token = extract_auth_header(request);
        let supplied = auth_token.as_deref().unwrap_or("");
        if supplied != cfg_token {
            let error_response = serde_json::json!({
                "success": false,
                "error": "Unauthorized: invalid token"
            });
            return Some(error_response.to_string());
        }
    }
    None
}

pub fn check_admin_authentication(request: &str, server_admin_token: &Option<String>) -> Option<String> {
    if let Some(cfg_admin_token) = server_admin_token {
        let auth_token = extract_auth_header(request);
        let supplied = auth_token.as_deref().unwrap_or("");
        if supplied != cfg_admin_token {
            let error_response = serde_json::json!({
                "success": false,
                "error": "Unauthorized: invalid admin token. JavaScript function management requires admin authentication."
            });
            return Some(error_response.to_string());
        }
    }
    None
}

pub struct TokenConfig {
    pub auth_token: Option<String>,
    pub admin_token: Option<String>,
    pub dev_mode_warning: bool,
    pub same_token_warning: bool,
    pub admin_inherited_warning: bool,
    pub eval_inherited_warning: bool,
}

impl TokenConfig {
    pub fn new(mut auth_token: Option<String>, mut admin_token: Option<String>) -> Self {
        let mut dev_mode_warning = false;
        let mut same_token_warning = false;
        let mut admin_inherited_warning = false;
        let mut eval_inherited_warning = false;

        match (&auth_token, &admin_token) {
            // No tokens provided - development mode
            (None, None) => {
                dev_mode_warning = true;
            }
            // Only eval token provided - admin inherits eval token
            (Some(eval_tok), None) => {
                admin_token = Some(eval_tok.clone());
                admin_inherited_warning = true;
            }
            // Only admin token provided - eval inherits admin token
            (None, Some(admin_tok)) => {
                auth_token = Some(admin_tok.clone());
                eval_inherited_warning = true;
            }
            // Both tokens provided - check if they're the same
            (Some(eval_tok), Some(admin_tok)) => {
                if eval_tok == admin_tok {
                    same_token_warning = true;
                }
            }
        }

        Self {
            auth_token,
            admin_token,
            dev_mode_warning,
            same_token_warning,
            admin_inherited_warning,
            eval_inherited_warning,
        }
    }

    pub fn print_warnings(&self) {
        if self.dev_mode_warning {
            eprintln!("⚠️  WARNING: Running in DEVELOPMENT MODE - no authentication required!");
            eprintln!("⚠️  This server is UNPROTECTED and should not be exposed to networks.");
        }
        if self.same_token_warning {
            eprintln!("⚠️  WARNING: Admin token and eval token are the same!");
            eprintln!("⚠️  Consider using different tokens for better security separation.");
        }
        if self.admin_inherited_warning {
            eprintln!("⚠️  WARNING: Admin token inherited from eval token!");
            eprintln!("⚠️  Admin operations use the same token as eval operations.");
        }
        if self.eval_inherited_warning {
            eprintln!("⚠️  WARNING: Eval token inherited from admin token!");
            eprintln!("⚠️  The same token has all privileges (eval and admin).");
        }
    }
}