use crate::udp::State;
use enigo::{Button, Coordinate, Direction, Enigo, Mouse};
use rquickjs::{
    function::Async,
    promise::MaybePromise,
    AsyncContext,
    AsyncRuntime,
    Error,
    Function,
    Object,
    Persistent,
    Value,
};
use std::{cell::RefCell, rc::Rc, time::Duration};

trait MouseInput {
    fn click_at(
        &mut self,
        x: i32,
        y: i32,
        button: Button,
    ) -> Result<(), String>;
}

impl MouseInput for Enigo {
    fn click_at(
        &mut self,
        x: i32,
        y: i32,
        button: Button,
    ) -> Result<(), String> {
        self.move_mouse(x, y, Coordinate::Abs)
            .map_err(|error| error.to_string())?;
        self.button(button, Direction::Click)
            .map_err(|error| error.to_string())
    }
}

type SharedMouse = Rc<RefCell<dyn MouseInput>>;

struct ScriptSession {
    // Rust drops fields in declaration order. Persistent roots must be gone
    // before their context and runtime.
    script: Persistent<Function<'static>>,
    memory: Persistent<Object<'static>>,
    context: AsyncContext,
    _runtime: AsyncRuntime,
}

impl ScriptSession {
    async fn new(source: &str) -> rquickjs::Result<Self> {
        let runtime = AsyncRuntime::new()?;
        let context = AsyncContext::full(&runtime).await?;
        let source = source.to_owned();

        let (script, memory) = context
            .async_with(async move |ctx| {
                let script: Function = ctx.eval(source)?;
                let memory = Object::new(ctx.clone())?;

                Ok::<_, rquickjs::Error>((
                    Persistent::save(&ctx, script),
                    Persistent::save(&ctx, memory),
                ))
            })
            .await?;

        Ok(Self {
            script,
            memory,
            context,
            _runtime: runtime,
        })
    }

    async fn invoke(
        &self,
        state: State,
        mouse: SharedMouse,
    ) -> rquickjs::Result<()> {
        let script = self.script.clone();
        let memory = self.memory.clone();

        self.context
            .async_with(async move |ctx| {
                let script: Function = script.restore(&ctx)?;
                let memory: Object = memory.restore(&ctx)?;

                let state_object = Object::new(ctx.clone())?;
                match state.score {
                    Some(score) => state_object.set("score", score)?,
                    None => state_object.set("score", Value::new_null(ctx.clone()))?,
                }
                state_object.set("sequence", state.sequence)?;
                match state.received_at_ms {
                    Some(received_at_ms) => {
                        state_object.set("receivedAtMs", received_at_ms)?
                    }
                    None => state_object
                        .set("receivedAtMs", Value::new_null(ctx.clone()))?,
                }

                let rev = Object::new(ctx.clone())?;
                rev.set("state", state_object.clone())?;

                let click_mouse = mouse.clone();
                rev.set(
                    "click",
                    Function::new(
                        ctx.clone(),
                        move |x: f64, y: f64, button: Option<String>| {
                            if !x.is_finite()
                                || x.fract() != 0.0
                                || x < i32::MIN as f64
                                || x > i32::MAX as f64
                                || !y.is_finite()
                                || y.fract() != 0.0
                                || y < i32::MIN as f64
                                || y > i32::MAX as f64
                            {
                                return Err(Error::new_from_js_message(
                                    "number",
                                    "finite 32-bit integer coordinates",
                                    "invalid mouse coordinates",
                                ));
                            }

                            let button = match button.as_deref().unwrap_or("left") {
                                "left" => Button::Left,
                                "right" => Button::Right,
                                "middle" => Button::Middle,
                                value => {
                                    return Err(Error::new_from_js_message(
                                        "string",
                                        "mouse button",
                                        format!("unsupported button: {value}"),
                                    ));
                                }
                            };

                            click_mouse
                                .borrow_mut()
                                .click_at(x as i32, y as i32, button)
                                .map_err(|message| {
                                    Error::new_from_js_message(
                                        "mouse input",
                                        "JavaScript",
                                        message,
                                    )
                                })
                        },
                    )?,
                )?;

                rev.set(
                    "sleep",
                    Function::new(
                        ctx.clone(),
                        Async(|milliseconds: f64| async move {
                            if !milliseconds.is_finite()
                                || milliseconds < 0.0
                                || milliseconds.fract() != 0.0
                                || milliseconds >= u64::MAX as f64
                            {
                                return Err(Error::new_from_js_message(
                                    "number",
                                    "non-negative integer milliseconds",
                                    "invalid sleep duration",
                                ));
                            }

                            tokio::time::sleep(Duration::from_millis(
                                milliseconds as u64,
                            ))
                            .await;
                            Ok::<(), Error>(())
                        }),
                    )?,
                )?;

                let freeze: Function = ctx.eval("Object.freeze")?;
                let _: Object = freeze.call((state_object.clone(),))?;
                let _: Object = freeze.call((rev.clone(),))?;

                let result: MaybePromise = script.call((rev, memory))?;
                let _: Value = result.into_future().await?;
                Ok(())
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::udp::State;
    use enigo::Button;
    use std::{
        cell::RefCell,
        rc::Rc,
        time::{Duration, Instant},
    };

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Click {
        x: i32,
        y: i32,
        button: Button,
    }

    struct FakeMouse {
        clicks: Rc<RefCell<Vec<Click>>>,
    }

    impl MouseInput for FakeMouse {
        fn click_at(
            &mut self,
            x: i32,
            y: i32,
            button: Button,
        ) -> Result<(), String> {
            self.clicks.borrow_mut().push(Click { x, y, button });
            Ok(())
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn passes_fresh_state_and_preserves_memory_and_globals() {
        let source = r#"
            (async (rev, memory) => {
                if (!Object.isFrozen(rev) || !Object.isFrozen(rev.state)) {
                    throw new Error("rev and state must be frozen");
                }
                memory.count = (memory.count ?? 0) + 1;
                globalThis.count = (globalThis.count ?? 0) + 1;
                if (memory.count !== globalThis.count) {
                    throw new Error("persistent state mismatch");
                }
                if (rev.state.sequence !== memory.count) {
                    throw new Error("stale invocation state");
                }
                if (memory.count === 2) {
                    rev.click(-12, 34, "right");
                }
                const sequence = rev.state.sequence;
                await rev.sleep(1);
                if (rev.state.sequence !== sequence) {
                    throw new Error("state changed during invocation");
                }
            })
        "#;

        let session = ScriptSession::new(source).await.unwrap();
        let clicks = Rc::new(RefCell::new(Vec::new()));
        let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
            clicks: clicks.clone(),
        }));

        session
            .invoke(
                State {
                    score: Some("1e1".to_owned()),
                    sequence: 1,
                    received_at_ms: Some(10),
                },
                mouse.clone(),
            )
            .await
            .unwrap();
        session
            .invoke(
                State {
                    score: Some("2e2".to_owned()),
                    sequence: 2,
                    received_at_ms: Some(20),
                },
                mouse,
            )
            .await
            .unwrap();

        assert_eq!(
            clicks.borrow().as_slice(),
            &[Click {
                x: -12,
                y: 34,
                button: Button::Right,
            }]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn invocation_error_does_not_reset_memory() {
        let session = ScriptSession::new(
            r#"
                ((rev, memory) => {
                    memory.count = (memory.count ?? 0) + 1;
                    if (memory.count === 1) {
                        throw new Error("first call");
                    }
                    if (memory.count !== 2) {
                        throw new Error("memory was reset");
                    }
                })
            "#,
        )
        .await
        .unwrap();
        let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
            clicks: Rc::new(RefCell::new(Vec::new())),
        }));

        assert!(
            session
                .invoke(State::default(), mouse.clone())
                .await
                .is_err()
        );
        session.invoke(State::default(), mouse).await.unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn missing_state_fields_are_null() {
        let session = ScriptSession::new(
            r#"((rev) => {
                if (rev.state.score !== null || rev.state.receivedAtMs !== null) {
                    throw new Error("missing state fields must be null");
                }
            })"#,
        )
        .await
        .unwrap();
        let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
            clicks: Rc::new(RefCell::new(Vec::new())),
        }));

        session.invoke(State::default(), mouse).await.unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sleep_is_awaited_and_non_callable_source_is_rejected() {
        let session = ScriptSession::new(
            "(async (rev, memory) => { await rev.sleep(20); })",
        )
        .await
        .unwrap();
        let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
            clicks: Rc::new(RefCell::new(Vec::new())),
        }));
        let started = Instant::now();

        let invocation = session.invoke(State::default(), mouse);
        tokio::pin!(invocation);
        tokio::select! {
            result = &mut invocation => {
                panic!("sleep completed before yielding: {result:?}")
            }
            _ = tokio::time::sleep(Duration::from_millis(1)) => {}
        }
        invocation.await.unwrap();

        assert!(started.elapsed() >= Duration::from_millis(20));
        assert!(ScriptSession::new("42").await.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn invalid_host_arguments_throw_without_clicking() {
        let clicks = Rc::new(RefCell::new(Vec::new()));

        for source in [
            r#"((rev) => rev.click(1, 2, "side"))"#,
            r#"((rev) => rev.click(1.5, 2, "left"))"#,
            r#"((rev) => rev.sleep(-1))"#,
            r#"((rev) => rev.sleep(1.5))"#,
        ] {
            let session = ScriptSession::new(source).await.unwrap();
            let mouse: SharedMouse = Rc::new(RefCell::new(FakeMouse {
                clicks: clicks.clone(),
            }));
            assert!(session.invoke(State::default(), mouse).await.is_err());
        }

        assert!(clicks.borrow().is_empty());
    }
}
