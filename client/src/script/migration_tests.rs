use super::{
    bindings::{Button, HostControls, MouseInput, MouseInputError},
    control::SessionControl,
    session::ScriptSession,
    State,
};
use crate::{
    bridge::{ScriptUiEvent, ScriptUiEventKind, ScriptUiPublisher, WsConnection},
    window::WindowControl,
};
use std::{cell::RefCell, rc::Rc, time::Duration};

struct NoopMouse;

impl MouseInput for NoopMouse {
    fn click_at(&mut self, _x: i32, _y: i32, _button: Button) -> Result<(), MouseInputError> { Ok(()) }
}

struct NoopWindow;

impl WindowControl for NoopWindow {
    fn resize_client(&self, _width: i32, _height: i32) -> Result<(), String> { Ok(()) }
}

#[tokio::test(flavor = "current_thread")]
async fn two_runtime_migrated_history_ui_uses_element_states() {
    let source = r#"
      export function afterLoad() {
        rev.ui('lastHistory', { text: "Last Run: Loading...", states: { expanded: true, expandedText: "", collapsedText: "" } });
        rev.ui.lastHistory.setOnClick(function() {
          this.states.expanded = this.states.expanded !== true;
          this.text = this.states.expanded ? this.states.expandedText : this.states.collapsedText;
        });
      }
      export default function() {
        const history = rev.ui.lastHistory;
        const run = typeof rev.global.run === "number" ? rev.global.run + 1 : 1;
        rev.global.run = run;
        history.states.expandedText = `expanded-${run}`;
        history.states.collapsedText = `collapsed-${run}`;
        history.text = history.states.expanded === true ? history.states.expandedText : history.states.collapsedText;
      }
    "#;
    let publisher = ScriptUiPublisher::default();
    let mut snapshots = publisher.subscribe();
    let session = ScriptSession::new_with_connection_and_control(
        source,
        "migration-history.js",
        WsConnection::disconnected_for_test(),
        SessionControl::standalone(),
        publisher,
    ).await.unwrap();
    let host = HostControls {
        mouse: Rc::new(RefCell::new(NoopMouse)), window: Rc::new(NoopWindow),
        actions_paused: crate::app::ActionGate::default(),
    };
    let mut background = Box::pin(session.drive_background(host.clone()));
    session.run_after_load(host.clone()).await.unwrap();
    tokio::select! {
        _ = &mut background => panic!("background service stopped unexpectedly"),
        result = async {
            loop {
                if snapshots.borrow().elements.iter().find(|element| element.id == "lastHistory").is_some_and(|element| element.events.contains(&ScriptUiEventKind::Click)) { break; }
                snapshots.changed().await.unwrap();
            }
        } => result,
    }
    session.invoke(State::default(), host.clone()).await.unwrap();
    assert_eq!(snapshots.borrow().elements.iter().find(|element| element.id == "lastHistory").unwrap().text, "expanded-1");

    let snapshot = snapshots.borrow().clone();
    snapshots.borrow_and_update();
    let history = snapshot.elements.iter().find(|element| element.id == "lastHistory").unwrap();
    let dispatch = session.dispatch_ui_event(ScriptUiEvent {
        session_id: snapshot.session_id.unwrap(), element_id: "lastHistory".to_owned(),
        instance_id: history.instance_id, events_version: history.events_version,
        event: ScriptUiEventKind::Click,
    }, host.clone());
    tokio::pin!(dispatch);
    tokio::select! {
        _ = &mut background => panic!("background service stopped unexpectedly"),
        result = &mut dispatch => result.unwrap(),
    }
    session.invoke(State::default(), host.clone()).await.unwrap();
    assert_eq!(snapshots.borrow().elements.iter().find(|element| element.id == "lastHistory").unwrap().text, "collapsed-2");
    let snapshot = snapshots.borrow().clone();
    snapshots.borrow_and_update();
    let history = snapshot.elements.iter().find(|element| element.id == "lastHistory").unwrap();
    let dispatch = session.dispatch_ui_event(ScriptUiEvent {
        session_id: snapshot.session_id.unwrap(), element_id: "lastHistory".to_owned(),
        instance_id: history.instance_id, events_version: history.events_version,
        event: ScriptUiEventKind::Click,
    }, host.clone());
    tokio::pin!(dispatch);
    tokio::select! {
        _ = &mut background => panic!("background service stopped unexpectedly"),
        result = &mut dispatch => result.unwrap(),
    }
    session.invoke(State::default(), host).await.unwrap();
    assert_eq!(snapshots.borrow().elements.iter().find(|element| element.id == "lastHistory").unwrap().text, "expanded-3");
}

#[tokio::test(flavor = "current_thread")]
async fn two_runtime_migrated_demo_monitor_survives_pause() {
    let source = r#"
      export function afterLoad() {
        rev.ui('main', { text: "main-idle" });
        rev.ui('monitor', { text: "monitor-0" });
        rev.ui('button', { text: "click-0", states: { clicks: 0 } });
        rev.ui.button.setOnClick(function() {
          this.states.clicks = this.states.clicks + 1;
          this.text = `click-${this.states.clicks}`;
        });
        rev.daemon.monitor = async function() {
          let count = 0;
          while (true) {
            rev.ui.monitor.text = `monitor-${++count}`;
            await rev.sleep(1);
          }
        };
      }
      export default async function() {
        rev.ui.main.text = "main-started";
        await rev.sleep(1000);
        rev.ui.main.text = "main-finished";
      }
    "#;
    let publisher = ScriptUiPublisher::default();
    let mut snapshots = publisher.subscribe();
    let session_control = SessionControl::standalone();
    let session = ScriptSession::new_with_connection_and_control(
        source,
        "migration-demo.js",
        WsConnection::disconnected_for_test(),
        session_control.clone(),
        publisher,
    ).await.unwrap();
    let host = HostControls {
        mouse: Rc::new(RefCell::new(NoopMouse)), window: Rc::new(NoopWindow),
        actions_paused: crate::app::ActionGate::default(),
    };
    let mut background = Box::pin(session.drive_background(host.clone()));
    session.run_after_load(host.clone()).await.unwrap();
    tokio::select! {
        _ = &mut background => panic!("background service stopped unexpectedly"),
        _ = tokio::time::sleep(Duration::from_millis(5)) => {}
    }
    let mut invocation = Box::pin(session.invoke(State::default(), host.clone()));
    tokio::select! {
        _ = &mut background => panic!("background service stopped unexpectedly"),
        result = async {
            loop {
                if snapshots.borrow().elements.iter().any(|element| element.id == "main" && element.text == "main-started") { break; }
                snapshots.changed().await.unwrap();
            }
        } => result,
        result = &mut invocation => panic!("main invocation completed early: {:?}", result),
    }
    session_control.acknowledge_pause(true);
    let main_at_pause = snapshots.borrow().elements.iter().find(|element| element.id == "main").unwrap().text.clone();
    let monitor_at_pause = snapshots.borrow().elements.iter().find(|element| element.id == "monitor").unwrap().text.clone();
    tokio::select! {
        _ = &mut background => panic!("background service stopped unexpectedly"),
        _ = tokio::time::sleep(Duration::from_millis(20)) => {}
    }
    assert_eq!(snapshots.borrow().elements.iter().find(|element| element.id == "main").unwrap().text, main_at_pause);
    assert_ne!(snapshots.borrow().elements.iter().find(|element| element.id == "monitor").unwrap().text, monitor_at_pause);

    let snapshot = snapshots.borrow().clone();
    let button = snapshot.elements.iter().find(|element| element.id == "button").unwrap();
    let dispatch = session.dispatch_ui_event(ScriptUiEvent {
        session_id: snapshot.session_id.unwrap(), element_id: "button".to_owned(),
        instance_id: button.instance_id, events_version: button.events_version,
        event: ScriptUiEventKind::Click,
    }, host);
    tokio::pin!(dispatch);
    tokio::select! {
        _ = &mut background => panic!("background service stopped unexpectedly"),
        result = &mut dispatch => result.unwrap(),
    }
    assert_eq!(snapshots.borrow().elements.iter().find(|element| element.id == "button").unwrap().text, "click-1");
    session_control.acknowledge_pause(false);
    invocation.await.unwrap();
}
