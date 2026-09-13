    #[test]
    fn verifier_optional_root_guards_reject_only_their_invalid_cases() {
        for (env, rejected) in [(None, false), (Some("ENV_HOME"), false), (Some(""), true), (Some(" \t\n"), true), (Some("\u{2003}"), true)] {
            let mut value: serde_json::Value = serde_json::from_str(&minimal_valid_json()).unwrap();
            value["roots"][0]["env_var"] = env.map_or(serde_json::Value::Null, |s| serde_json::Value::String(s.into()));
            let actual = parse_manifest(&value.to_string());
            if rejected { assert!(matches!(actual, Err(ManifestError::EmptyEnvVarName))); } else { assert!(actual.is_ok(), "{env:?}"); }
        }
        for (subdir, rejected) in [(None, false), (Some("cache"), false), (Some("cache/sessions"), false), (Some(""), true), (Some(".."), true), (Some("../escape"), true), (Some("/etc"), true)] {
            let mut value: serde_json::Value = serde_json::from_str(&minimal_valid_json()).unwrap();
            value["roots"][0]["subdir"] = subdir.map_or(serde_json::Value::Null, |s| serde_json::Value::String(s.into()));
            let actual = parse_manifest(&value.to_string());
            if rejected { assert!(matches!(actual, Err(ManifestError::PathEscapesRoot(_)))); } else { assert!(actual.is_ok(), "{subdir:?}"); }
        }
    }
