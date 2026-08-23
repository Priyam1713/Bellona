use foedus::{from_bus, AgUiEvent, AgentCard};
use forge::event::BusEvent;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[test]
fn bus_events_translate_to_surface_stream() {
    let veto = BusEvent::VetoRaised {
        reason: "test".into(),
    };
    match from_bus(&veto) {
        Some(AgUiEvent::Error { message }) => assert!(message.contains("VETO")),
        other => panic!("veto must surface: {other:?}"),
    }

    let denied = BusEvent::DecisionMade {
        action_id: forge::ActionId::mint(),
        verdict: "deny".into(),
        rule_id: "no-writes".into(),
    };
    match from_bus(&denied) {
        Some(AgUiEvent::Error { message }) => assert!(message.contains("no-writes")),
        other => panic!("denial must surface with rule name: {other:?}"),
    }

    // Pre-decision events stay internal.
    let requested = BusEvent::ActionRequested {
        action_id: forge::ActionId::mint(),
    };
    assert!(from_bus(&requested).is_none());
}

#[tokio::test]
async fn fanout_reaches_every_attached_surface() {
    #[derive(Default)]
    struct Sink(Arc<Mutex<Vec<String>>>);
    #[async_trait::async_trait]
    impl foedus::AgUiEmitter for Sink {
        async fn emit(&self, ev: foedus::AgUiEvent) -> Result<(), foedus::FoedusError> {
            self.0.lock().unwrap().push(format!("{ev:?}"));
            Ok(())
        }
    }

    let mut fan = foedus::AgUiFanout::new();
    let s1 = Arc::new(Sink::default());
    let s2 = Arc::new(Sink::default());
    fan.attach(s1.clone());
    fan.attach(s2.clone());
    fan.broadcast(AgUiEvent::RunFinished {
        run_id: "r1".into(),
        ok: true,
    })
    .await;

    assert_eq!(s1.0.lock().unwrap().len(), 1);
    assert_eq!(s2.0.lock().unwrap().len(), 1);
}

#[test]
fn agent_cards_round_trip() {
    let card = AgentCard {
        name: "bellona-centurion".into(),
        description: "delegates campaigns".into(),
        skills: vec!["research".into(), "code-review".into()],
        endpoint: "https://camp.example/a2a".into(),
        protocol_versions: foedus::PROTOCOL_VERSIONS
            .iter()
            .map(|s| s.to_string())
            .collect(),
    };
    let json = serde_json::to_string(&card).unwrap();
    let back: AgentCard = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, "bellona-centurion");
    assert_eq!(back.skills.len(), 2);

    // Silence unused-import lint for attrs map in some toolchains.
    let _ = BTreeMap::<String, String>::new();
}
