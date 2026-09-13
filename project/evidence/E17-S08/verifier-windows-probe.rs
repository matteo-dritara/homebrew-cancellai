#[derive(Debug, PartialEq)]
struct ProcessObservation { complete: bool, running_names: Vec<String> }
thread_local! { static INPUT: std::cell::RefCell<Result<Vec<String>,()>> = const { std::cell::RefCell::new(Err(())) }; }
mod cancellai_sealedfs { pub fn list_running_process_names()->Result<Vec<String>,()> { super::INPUT.with(|s|s.borrow().clone()) } }
fn observe_system_processes(names: &[&str]) -> ProcessObservation {
    let Ok(running) = cancellai_sealedfs::list_running_process_names() else {
        return ProcessObservation {
            complete: false,
            running_names: Vec::new(),
        };
    };
    let mut running_names = Vec::new();
    for exe_name in running {
        let base = exe_name
            .strip_suffix(".exe")
            .or_else(|| exe_name.strip_suffix(".EXE"))
            .unwrap_or(&exe_name);
        if let Some(&matched) = names.iter().find(|n| n.eq_ignore_ascii_case(base))
            && !running_names.iter().any(|n: &String| n == matched)
        {
            running_names.push(matched.to_string());
        }
    }
    ProcessObservation {
        complete: true,
        running_names,
    }
}


#[test] fn known_names_are_case_insensitive_and_unique() {
 INPUT.with(|s|*s.borrow_mut()=Ok(vec!["CLAUDE.exe".into(),"claude.EXE".into(),"codex.exe".into(),"unknown.exe".into()]));
 assert_eq!(observe_system_processes(&["claude","codex"]),ProcessObservation{complete:true,running_names:vec!["claude".into(),"codex".into()]});
 assert_eq!(observe_system_processes(&[]),ProcessObservation{complete:true,running_names:vec![]});
 assert_eq!(observe_system_processes(&["missing"]),ProcessObservation{complete:true,running_names:vec![]});
 assert_eq!(observe_system_processes(&["claude","claude"]),ProcessObservation{complete:true,running_names:vec!["claude".into()]});
}
#[test] fn failed_enumeration_is_unknown() { INPUT.with(|s|*s.borrow_mut()=Err(()));assert_eq!(observe_system_processes(&["claude"]),ProcessObservation{complete:false,running_names:vec![]}); }
