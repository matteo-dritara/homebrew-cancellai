    #[test]
    fn verifier_dump_rendered_screens() {
        let out = std::env::var("CANCELLAI_REVIEW_RENDER_DIR").unwrap();
        std::fs::create_dir_all(&out).unwrap();
        let data = EngineData { atlas: Some(sample_summary()), explain: sample_explain_views() };
        for screen in SCREENS {
            for (width, height) in [(100, 30), (60, 15), (24, 6), (23, 5), (5, 2)] {
                for unicode in [false, true] {
                    for help in [false, true] {
                        let mut app = App::new(); app.screen = screen; app.show_help = help;
                        let cap = TerminalCapability { color: ColorSupport::Basic, unicode };
                        let mut terminal = Terminal::new(TestBackend::new(width,height)).unwrap();
                        terminal.draw(|frame| draw(frame, &app, cap, &data)).unwrap();
                        let content = terminal.backend().buffer().content().chunks(usize::from(width))
                            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>()).collect::<Vec<_>>().join("\n");
                        let name = format!("{screen:?}-{width}x{height}-unicode{unicode}-help{help}.txt");
                        std::fs::write(std::path::Path::new(&out).join(name),content).unwrap();
                    }
                }
            }
        }
    }
