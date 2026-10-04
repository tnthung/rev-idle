use super::{
    background::{BackgroundRegistry, BackgroundRegistrationStatus, BackgroundTarget},
    bindings::{bridge_error, reject_if_stopped},
    control::SessionControl,
    transfer::FunctionTransfer,
    value::EncodedValue,
};
use crate::bridge::{
    ScriptUiBorderState, ScriptUiCornerState, ScriptUiElementState, ScriptUiEvent,
    ScriptUiEventKind, ScriptUiLengthState, ScriptUiMeasureReq, ScriptUiPaddingState,
    ScriptUiPublisher, WsConnection,
};
use rquickjs::{
    function::Async, Ctx, Exception, Function, IntoJs, Object, Persistent, Value,
};
use serde_json::{Map, Value as JsonValue};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use uuid::Uuid;

#[derive(Clone)]
struct UiHandler {
    registration: Uuid,
    active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UiEventKind {
    Pointer(ScriptUiEventKind),
    StateUpdate,
}

struct UiElement {
    name: String,
    instance: Uuid,
    values: Map<String, JsonValue>,
    states: BTreeMap<String, EncodedValue>,
    events_version: u64,
    handlers: [Option<UiHandler>; 4],
}

struct UiStore {
    elements: Vec<UiElement>,
}

pub(super) struct ScriptUiState {
    session_id: Uuid,
    publisher: ScriptUiPublisher,
    session: SessionControl,
    store: RefCell<UiStore>,
    pub(super) state_updates: RefCell<Vec<Uuid>>,
}

pub(super) struct ScriptUiBindings {
    state: Rc<ScriptUiState>,
    api: Persistent<Object<'static>>,
}

fn ui_error(message: impl Into<String>) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("UI", "value", message.into())
}

fn event_index(event: UiEventKind) -> usize {
    match event {
        UiEventKind::Pointer(ScriptUiEventKind::Hover) => 0,
        UiEventKind::Pointer(ScriptUiEventKind::Leave) => 1,
        UiEventKind::Pointer(ScriptUiEventKind::Click) => 2,
        UiEventKind::StateUpdate => 3,
    }
}

fn parse_event(value: &str) -> Result<UiEventKind, String> {
    match value {
        "hover" => Ok(UiEventKind::Pointer(ScriptUiEventKind::Hover)),
        "leave" => Ok(UiEventKind::Pointer(ScriptUiEventKind::Leave)),
        "click" => Ok(UiEventKind::Pointer(ScriptUiEventKind::Click)),
        "stateUpdate" => Ok(UiEventKind::StateUpdate),
        _ => Err(format!("unknown UI event: {value}")),
    }
}

fn default_value(field: &str) -> Option<JsonValue> {
    Some(match field {
        "hidden" => JsonValue::Bool(false),
        "basedOn" => JsonValue::String(String::new()),
        "text" => JsonValue::String(String::new()),
        "font" => JsonValue::String(String::new()),
        "size" => JsonValue::Number(14.into()),
        "posX" | "posY" => JsonValue::Number(serde_json::Number::from_f64(0.0).unwrap()),
        "lenX" | "lenY" => serde_json::json!({ "min": 0.0 }),
        "alignX" => JsonValue::String("left".to_owned()),
        "alignY" => JsonValue::String("center".to_owned()),
        "color" => serde_json::json!([0, 0, 0, 0]),
        "textColor" => serde_json::json!([255, 255, 255, 255]),
        "border" => serde_json::json!({ "thickness": 0.0, "color": [255, 255, 255] }),
        "corner" => serde_json::json!({ "radius": 0.0 }),
        "padding" => serde_json::json!({ "thickness": 0.0 }),
        _ => return None,
    })
}

fn finite_number(value: &JsonValue, label: &str) -> Result<f64, String> {
    let value = value.as_f64().ok_or_else(|| format!("{label} must be a number"))?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("invalid {label}"))
    }
}

fn validate_color(value: &JsonValue, label: &str) -> Result<JsonValue, String> {
    let values = value.as_array().ok_or_else(|| format!("invalid {label}"))?;
    if values.len() != 3 && values.len() != 4 {
        return Err(format!("invalid {label}"));
    }
    for channel in values {
        let number = channel.as_u64().ok_or_else(|| format!("invalid {label} channel"))?;
        if number > 255 {
            return Err(format!("invalid {label} channel"));
        }
    }
    Ok(value.clone())
}

fn validate_compound(field: &str, value: &JsonValue) -> Result<JsonValue, String> {
    let values = value.as_object().ok_or_else(|| format!("invalid {field}"))?;
    let allowed = match field {
        "lenX" | "lenY" => ["min", "max"].as_slice(),
        "border" => ["thickness", "color"].as_slice(),
        "corner" => ["radius", "topLeft", "topRight", "bottomLeft", "bottomRight"].as_slice(),
        "padding" => ["thickness", "top", "right", "bottom", "left"].as_slice(),
        _ => return Err(format!("invalid {field}")),
    };
    for (key, item) in values {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("Unknown {field} field: {key}"));
        }
        if field == "border" && key == "color" {
            validate_color(item, "border.color")?;
        } else {
            let value = finite_number(item, &format!("{field}.{key}"))?;
            if value < 0.0 {
                return Err(format!("invalid {field}.{key}"));
            }
        }
    }
    if (field == "lenX" || field == "lenY")
        && values.get("max").is_some()
        && finite_number(values.get("max").unwrap(), "length.max")?
            < values.get("min").map_or(0.0, |value| finite_number(value, "length.min").unwrap_or(-1.0))
    {
        return Err("Maximum length is less than minimum".to_owned());
    }
    Ok(value.clone())
}

fn validate_field(field: &str, value: &JsonValue) -> Result<JsonValue, String> {
    if matches!(field, "onClick" | "onHover" | "onLeave" | "onStateUpdate") {
        return Err(format!("{field} is configured through its setter"));
    }
    if field == "states" {
        return value
            .as_object()
            .map(|_| value.clone())
            .ok_or_else(|| "states must be an object".to_owned());
    }
    if default_value(field).is_none() {
        return Err(format!("Unknown UI field: {field}"));
    }
    match field {
        "hidden" => value
            .as_bool()
            .map(JsonValue::Bool)
            .ok_or_else(|| "hidden must be a boolean".to_owned()),
        "basedOn" | "text" | "font" => value
            .as_str()
            .map(|value| JsonValue::String(value.to_owned()))
            .ok_or_else(|| format!("{field} must be a string")),
        "size" => {
            let value = value.as_i64().ok_or_else(|| "Invalid size".to_owned())?;
            if !(1..=2_147_483_647).contains(&value) {
                return Err("Invalid size".to_owned());
            }
            Ok(value.into())
        }
        "alignX" if matches!(value.as_str(), Some("left" | "center" | "right")) => Ok(value.clone()),
        "alignY" if matches!(value.as_str(), Some("top" | "center" | "bottom")) => Ok(value.clone()),
        "alignX" | "alignY" => Err(format!("Invalid {field}")),
        "posX" | "posY" => finite_number(value, field).map(|_| value.clone()),
        "lenX" | "lenY" if value.is_number() => {
            let value = finite_number(value, field)?;
            if value < 0.0 {
                return Err(format!("Invalid {field}"));
            }
            Ok(JsonValue::Number(serde_json::Number::from_f64(value).unwrap()))
        }
        "color" | "textColor" => validate_color(value, field),
        "lenX" | "lenY" | "border" | "corner" | "padding" => validate_compound(field, value),
        _ => Err(format!("Invalid {field}")),
    }
}

fn split_states(value: &JsonValue, big_nums: &[Vec<String>], colors: &[Vec<String>], has_states_prefix: bool) -> Result<BTreeMap<String, EncodedValue>, String> {
    let fields = value.as_object().ok_or_else(|| "states must be an object".to_owned())?;
    let mut states = fields.iter().map(|(key, value)| (key.clone(), EncodedValue { value: value.clone(), big_nums: Vec::new(), colors: Vec::new() })).collect::<BTreeMap<_, _>>();
    for (paths, color) in [(big_nums, false), (colors, true)] {
        for path in paths {
            let (key, offset) = if has_states_prefix {
                if path.first().map(String::as_str) != Some("states") { return Err("Invalid native metadata in UI attributes".to_owned()); }
                (path.get(1).ok_or_else(|| "Invalid native value path".to_owned())?, 2)
            } else {
                (path.first().ok_or_else(|| "Invalid native value path".to_owned())?, 1)
            };
            let state = states.get_mut(key).ok_or_else(|| "Native value path does not resolve to a state".to_owned())?;
            if color { state.colors.push(path[offset..].to_vec()); }
            else { state.big_nums.push(path[offset..].to_vec()); }
        }
    }
    Ok(states)
}

fn validate_definition(json: &str) -> Result<(Map<String, JsonValue>, BTreeMap<String, EncodedValue>, bool), String> {
    let encoded: EncodedValue = serde_json::from_str(json).map_err(|error| error.to_string())?;
    if encoded.big_nums.iter().chain(&encoded.colors).any(|path| path.first().map(String::as_str) != Some("states")) {
        return Err("Native values are not valid UI attributes".to_owned());
    }
    let value = encoded.value;
    let fields = value
        .as_object()
        .ok_or_else(|| "UI definition must be an object".to_owned())?;
    let mut values = Map::new();
    let mut states = BTreeMap::new();
    let mut has_states = false;
    for (field, value) in fields {
        if field == "states" {
            has_states = true;
            states = split_states(value, &encoded.big_nums, &encoded.colors, true)?;
        } else {
            values.insert(field.clone(), validate_field(field, value)?);
        }
    }
    Ok((values, states, has_states))
}

fn value_number(values: &Map<String, JsonValue>, field: &str, default: f64) -> f64 {
    values.get(field).and_then(JsonValue::as_f64).unwrap_or(default)
}

fn value_string(values: &Map<String, JsonValue>, field: &str, default: &str) -> String {
    values.get(field).and_then(JsonValue::as_str).unwrap_or(default).to_owned()
}

fn snapshot_element(element: &UiElement) -> ScriptUiElementState {
    let value = |field: &str| element.values.get(field).cloned().or_else(|| default_value(field)).unwrap();
    let length = |field: &str| match value(field) {
        JsonValue::Number(number) => ScriptUiLengthState { fixed: number.as_f64(), min: 0.0, max: None },
        JsonValue::Object(values) => ScriptUiLengthState {
            fixed: None,
            min: values.get("min").and_then(JsonValue::as_f64).unwrap_or(0.0),
            max: values.get("max").and_then(JsonValue::as_f64),
        },
        _ => unreachable!(),
    };
    let color = |field: &str| {
        let values = value(field).as_array().unwrap().clone();
        [
            values[0].as_u64().unwrap() as u8,
            values[1].as_u64().unwrap() as u8,
            values[2].as_u64().unwrap() as u8,
            values.get(3).and_then(JsonValue::as_u64).unwrap_or(255) as u8,
        ]
    };
    let border = value("border").as_object().unwrap().clone();
    let border_color = border.get("color").map_or([255, 255, 255, 255], |value| {
        let values = value.as_array().unwrap();
        [
            values[0].as_u64().unwrap() as u8,
            values[1].as_u64().unwrap() as u8,
            values[2].as_u64().unwrap() as u8,
            values.get(3).and_then(JsonValue::as_u64).unwrap_or(255) as u8,
        ]
    });
    let border_thickness = border.get("thickness").and_then(JsonValue::as_f64).unwrap_or(0.0);
    let corner = value("corner").as_object().unwrap().clone();
    let corner_radius = corner.get("radius").and_then(JsonValue::as_f64).unwrap_or(0.0);
    let corner_value = |field: &str| corner.get(field).and_then(JsonValue::as_f64).unwrap_or(corner_radius);
    let padding = value("padding").as_object().unwrap().clone();
    let padding_thickness = padding.get("thickness").and_then(JsonValue::as_f64).unwrap_or(0.0);
    let padding_value = |field: &str| padding.get(field).and_then(JsonValue::as_f64).unwrap_or(padding_thickness);
    let mut events = Vec::new();
    for event in [ScriptUiEventKind::Hover, ScriptUiEventKind::Leave, ScriptUiEventKind::Click] {
        if element.handlers[event_index(UiEventKind::Pointer(event))].as_ref().is_some_and(|handler| handler.active) {
            events.push(event);
        }
    }
    ScriptUiElementState {
        id: element.name.clone(),
        instance_id: element.instance,
        events_version: element.events_version,
        hidden: value("hidden").as_bool().unwrap(),
        based_on: value_string(&element.values, "basedOn", ""),
        text: value_string(&element.values, "text", ""),
        font: value_string(&element.values, "font", ""),
        size: value("size").as_i64().unwrap() as i32,
        align_x: value_string(&element.values, "alignX", "left"),
        align_y: value_string(&element.values, "alignY", "center"),
        pos_x: value_number(&element.values, "posX", 0.0),
        pos_y: value_number(&element.values, "posY", 0.0),
        len_x: length("lenX"),
        len_y: length("lenY"),
        color: color("color"),
        text_color: color("textColor"),
        border: ScriptUiBorderState { thickness: border_thickness, color: border_color },
        corner: ScriptUiCornerState {
            top_left: corner_value("topLeft"),
            top_right: corner_value("topRight"),
            bottom_left: corner_value("bottomLeft"),
            bottom_right: corner_value("bottomRight"),
        },
        padding: ScriptUiPaddingState {
            top: padding_value("top"),
            right: padding_value("right"),
            bottom: padding_value("bottom"),
            left: padding_value("left"),
        },
        events,
    }
}

impl ScriptUiState {
    pub(super) fn new(publisher: ScriptUiPublisher, session: SessionControl) -> Self {
        let state = Self {
            session_id: Uuid::new_v4(),
            publisher,
            session,
            store: RefCell::new(UiStore { elements: Vec::new() }),
            state_updates: RefCell::new(Vec::new()),
        };
        state.publisher.replace(state.session_id, Vec::new());
        state
    }

    pub(super) fn session_id(&self) -> Uuid {
        self.session_id
    }

    fn check_live(&self) -> Result<(), String> {
        if self.session.is_stopped() {
            Err(SessionControl::error_message().to_owned())
        } else {
            Ok(())
        }
    }

    fn publish(&self) {
        let elements = {
            let store = self.store.borrow();
            store.elements.iter().map(snapshot_element).collect()
        };
        self.publisher.replace(self.session_id, elements);
    }

    fn element_index(store: &UiStore, name: &str, instance: Option<Uuid>) -> Result<usize, String> {
        let index = store.elements.iter().position(|element| element.name == name);
        let Some(index) = index else { return Err("UI element no longer exists".to_owned()) };
        if instance.is_some_and(|instance| store.elements[index].instance != instance) {
            return Err("UI element no longer exists".to_owned());
        }
        Ok(index)
    }

    fn checked(&self, name: &str, instance: Uuid) -> Result<usize, String> {
        self.check_live()?;
        Self::element_index(&self.store.borrow(), name, Some(instance))
    }

    pub(super) fn lookup(&self, name: &str) -> Option<Uuid> {
        self.store.borrow().elements.iter().find(|element| element.name == name).map(|element| element.instance)
    }

    pub(super) fn names(&self) -> Vec<String> {
        self.store.borrow().elements.iter().map(|element| element.name.clone()).collect()
    }

    pub(super) fn record_json(&self, name: &str, instance: Uuid) -> Result<String, String> {
        let encoded = {
            let store = self.store.borrow();
            let element = &store.elements[Self::element_index(&store, name, Some(instance))?];
            let mut states = Map::new();
            let mut big_nums = Vec::new();
            let mut colors = Vec::new();
            for (key, encoded) in &element.states {
                states.insert(key.clone(), encoded.value.clone());
                for (paths, result) in [(&encoded.big_nums, &mut big_nums), (&encoded.colors, &mut colors)] {
                    for path in paths {
                        let mut full_path = vec!["states".to_owned(), key.clone()];
                        full_path.extend(path.iter().cloned());
                        result.push(full_path);
                    }
                }
            }
            EncodedValue { value: serde_json::json!({ "values": element.values, "states": states }), big_nums, colors }
        };
        serde_json::to_string(&encoded)
            .map_err(|error| error.to_string())
    }

    pub(super) fn define(&self, name: &str, json: &str) -> Result<Uuid, String> {
        self.check_live()?;
        if name.is_empty() {
            return Err("UI names must be nonempty strings".to_owned());
        }
        let (values, states, has_states) = validate_definition(json)?;
        let (instance, changed) = {
            let mut store = self.store.borrow_mut();
            if let Some(element) = store.elements.iter_mut().find(|element| element.name == name) {
                let mut changed = false;
                for (field, value) in values {
                    if element.values.get(&field).cloned().or_else(|| default_value(&field)) != Some(value.clone()) {
                        element.values.insert(field, value);
                        changed = true;
                    }
                }
                if has_states && element.states != states {
                        element.states = states;
                        if let Some(handler) = &element.handlers[event_index(UiEventKind::StateUpdate)] {
                            self.state_updates.borrow_mut().push(handler.registration);
                        }
                }
                (element.instance, changed)
            } else {
                let instance = Uuid::new_v4();
                store.elements.push(UiElement {
                    name: name.to_owned(), instance, values, states, events_version: 1,
                    handlers: [None, None, None, None],
                });
                (instance, true)
            }
        };
        if changed { self.publish(); }
        Ok(instance)
    }

    pub(super) fn remove(&self, name: &str) -> Result<Option<(Uuid, Vec<UiEventKind>)>, String> {
        self.check_live()?;
        let old = {
            let mut store = self.store.borrow_mut();
            let Some(index) = store.elements.iter().position(|element| element.name == name) else { return Ok(None) };
            let element = store.elements.remove(index);
            (element.instance, [UiEventKind::Pointer(ScriptUiEventKind::Hover), UiEventKind::Pointer(ScriptUiEventKind::Leave), UiEventKind::Pointer(ScriptUiEventKind::Click), UiEventKind::StateUpdate]
                .into_iter().filter(|event| element.handlers[event_index(*event)].is_some()).collect())
        };
        self.publish();
        Ok(Some(old))
    }

    pub(super) fn set_field(&self, name: &str, instance: Uuid, field: &str, json: &str) -> Result<bool, String> {
        self.check_live()?;
        let encoded: EncodedValue = serde_json::from_str(json).map_err(|error| error.to_string())?;
        if field == "states" {
            let states = split_states(&encoded.value, &encoded.big_nums, &encoded.colors, false)?;
            let changed = {
                let mut store = self.store.borrow_mut();
                let index = Self::element_index(&store, name, Some(instance))?;
                let element = &mut store.elements[index];
                if element.states == states { false } else {
                    element.states = states;
                    if let Some(handler) = &element.handlers[event_index(UiEventKind::StateUpdate)] {
                        self.state_updates.borrow_mut().push(handler.registration);
                    }
                    true
                }
            };
            return Ok(changed);
        }
        if !encoded.big_nums.is_empty() || !encoded.colors.is_empty() {
            return Err("Native values are not valid UI attributes".to_owned());
        }
        let value = validate_field(field, &encoded.value)?;
        let changed = {
            let mut store = self.store.borrow_mut();
            let index = Self::element_index(&store, name, Some(instance))?;
            let element = &mut store.elements[index];
            if element.values.get(field).cloned().or_else(|| default_value(field)) == Some(value.clone()) {
                false
            } else {
                element.values.insert(field.to_owned(), value);
                true
            }
        };
        if changed { self.publish(); }
        Ok(changed)
    }

    pub(super) fn delete_field(&self, name: &str, instance: Uuid, field: &str) -> Result<bool, String> {
        self.check_live()?;
        let changed = {
            let mut store = self.store.borrow_mut();
            let index = Self::element_index(&store, name, Some(instance))?;
            let element = &mut store.elements[index];
            if field == "states" {
                if element.states.is_empty() { false } else {
                    element.states.clear();
                    if let Some(handler) = &element.handlers[event_index(UiEventKind::StateUpdate)] {
                        self.state_updates.borrow_mut().push(handler.registration);
                    }
                    true
                }
            } else if default_value(field).is_some() {
                element.values.remove(field).is_some()
            } else {
                false
            }
        };
        if changed && field != "states" { self.publish(); }
        Ok(changed)
    }

    pub(super) fn state_keys(&self, name: &str, instance: Uuid) -> Result<Vec<String>, String> {
        let index = self.checked(name, instance)?;
        Ok(self.store.borrow().elements[index].states.keys().cloned().collect())
    }

    pub(super) fn state_json(&self, name: &str, instance: Uuid, key: &str) -> Result<Option<String>, String> {
        let (exists, value) = {
            let store = self.store.borrow();
            let element = &store.elements[Self::element_index(&store, name, Some(instance))?];
            (element.states.contains_key(key), element.states.get(key).cloned())
        };
        if !exists { return Ok(None); }
        serde_json::to_string(&value.unwrap()).map(Some).map_err(|error| error.to_string())
    }

    pub(super) fn set_state(&self, name: &str, instance: Uuid, key: &str, json: &str) -> Result<bool, String> {
        self.check_live()?;
        let value: EncodedValue = serde_json::from_str(json).map_err(|error| error.to_string())?;
        let mut store = self.store.borrow_mut();
        let index = Self::element_index(&store, name, Some(instance))?;
        let element = &mut store.elements[index];
        if element.states.get(key) == Some(&value) { return Ok(false); }
        element.states.insert(key.to_owned(), value);
        if let Some(handler) = &element.handlers[event_index(UiEventKind::StateUpdate)] {
            self.state_updates.borrow_mut().push(handler.registration);
        }
        Ok(true)
    }

    pub(super) fn delete_state(&self, name: &str, instance: Uuid, key: &str) -> Result<bool, String> {
        self.check_live()?;
        let mut store = self.store.borrow_mut();
        let index = Self::element_index(&store, name, Some(instance))?;
        let element = &mut store.elements[index];
        if element.states.remove(key).is_none() { return Ok(false); }
        if let Some(handler) = &element.handlers[event_index(UiEventKind::StateUpdate)] {
            self.state_updates.borrow_mut().push(handler.registration);
        }
        Ok(true)
    }

    pub(super) fn begin_handler_install(&self, name: &str, instance: Uuid, event: UiEventKind, registration: Uuid) -> Result<(), String> {
        self.check_live()?;
        {
            let mut store = self.store.borrow_mut();
            let index = Self::element_index(&store, name, Some(instance))?;
            let element = &mut store.elements[index];
            if event != UiEventKind::StateUpdate {
                element.events_version = element.events_version.checked_add(1).ok_or_else(|| "UI event version overflow".to_owned())?;
            }
            element.handlers[event_index(event)] = Some(UiHandler { registration, active: false });
        }
        if event != UiEventKind::StateUpdate { self.publish(); }
        Ok(())
    }

    pub(super) fn clear_handler(&self, name: &str, instance: Uuid, event: UiEventKind) -> Result<Option<Uuid>, String> {
        self.check_live()?;
        let registration = {
            let mut store = self.store.borrow_mut();
            let index = Self::element_index(&store, name, Some(instance))?;
            let element = &mut store.elements[index];
            let registration = element.handlers[event_index(event)].take().map(|handler| handler.registration);
            if registration.is_some() && event != UiEventKind::StateUpdate {
                element.events_version = element.events_version.checked_add(1).ok_or_else(|| "UI event version overflow".to_owned())?;
            }
            registration
        };
        if registration.is_some() && event != UiEventKind::StateUpdate { self.publish(); }
        Ok(registration)
    }

    pub(super) fn complete_handler_install(&self, name: &str, instance: Uuid, event: UiEventKind, registration: Uuid, installed: bool) -> bool {
        if self.session.is_stopped() { return false; }
        let current = {
            let mut store = self.store.borrow_mut();
            let Some(index) = store.elements.iter().position(|element| element.name == name) else { return false };
            let element = &mut store.elements[index];
            let Some(handler) = element.handlers[event_index(event)].as_mut() else { return false };
            if element.instance != instance || handler.registration != registration { return false; }
            handler.active = installed;
            true
        };
        if current && installed && event != UiEventKind::StateUpdate { self.publish(); }
        current
    }

    pub(super) fn installed_handler_registration(&self, event: &ScriptUiEvent) -> Option<Uuid> {
        if event.session_id != self.session_id || self.session.is_stopped() { return None; }
        let store = self.store.borrow();
        let element = store.elements.iter().find(|element| element.name == event.element_id && element.instance == event.instance_id)?;
        if element.events_version != event.events_version { return None; }
        element.handlers[event_index(UiEventKind::Pointer(event.event))].as_ref().filter(|handler| handler.active).map(|handler| handler.registration)
    }
}

impl Drop for ScriptUiState {
    fn drop(&mut self) {
        self.publisher.clear(self.session_id);
    }
}

impl ScriptUiBindings {
    pub(super) fn new<'js>(
        ctx: &Ctx<'js>,
        codec: Object<'js>,
        state: Rc<ScriptUiState>,
        connection: WsConnection,
        session: SessionControl,
        transfer: Rc<FunctionTransfer>,
        background: Rc<BackgroundRegistry>,
    ) -> rquickjs::Result<Self> {
        let host = Object::new(ctx.clone())?;
        let host_state = state.clone();
        host.set("lookup", Function::new(ctx.clone(), move |name: String| host_state.lookup(&name).map(|value| value.to_string()))?)?;
        let host_state = state.clone();
        host.set("names", Function::new(ctx.clone(), move || host_state.names())?)?;
        let host_session = session.clone();
        host.set("stopped", Function::new(ctx.clone(), move || host_session.is_stopped())?)?;
        let host_state = state.clone();
        host.set("record", Function::new(ctx.clone(), move |name: String, instance: String| {
            let instance = Uuid::parse_str(&instance).map_err(|error| ui_error(error.to_string()))?;
            host_state.record_json(&name, instance).map_err(ui_error)
        })?)?;
        let host_state = state.clone();
        host.set("define", Function::new(ctx.clone(), move |name: String, json: String| {
            host_state.define(&name, &json).map(|instance| instance.to_string()).map_err(ui_error)
        })?)?;
        let host_state = state.clone();
        let remove_background = background.clone();
        host.set("remove", Function::new(ctx.clone(), move |name: String| {
            host_state.remove(&name).map(|old| {
                if let Some((instance, events)) = old {
                    for event in events {
                        remove_background.unregister(&BackgroundTarget::Ui { name: name.clone(), instance, event });
                    }
                }
            }).map_err(ui_error)
        })?)?;
        let host_state = state.clone();
        host.set("setField", Function::new(ctx.clone(), move |name: String, instance: String, field: String, json: String| {
            let instance = Uuid::parse_str(&instance).map_err(|error| ui_error(error.to_string()))?;
            host_state.set_field(&name, instance, &field, &json).map(|_| ()).map_err(ui_error)
        })?)?;
        let host_state = state.clone();
        host.set("deleteField", Function::new(ctx.clone(), move |name: String, instance: String, field: String| {
            let instance = Uuid::parse_str(&instance).map_err(|error| ui_error(error.to_string()))?;
            host_state.delete_field(&name, instance, &field).map(|_| ()).map_err(ui_error)
        })?)?;
        let host_state = state.clone();
        host.set("stateKeys", Function::new(ctx.clone(), move |name: String, instance: String| {
            let instance = Uuid::parse_str(&instance).map_err(|error| ui_error(error.to_string()))?;
            host_state.state_keys(&name, instance).map_err(ui_error)
        })?)?;
        let host_state = state.clone();
        host.set("stateGet", Function::new(ctx.clone(), move |name: String, instance: String, key: String| {
            let instance = Uuid::parse_str(&instance).map_err(|error| ui_error(error.to_string()))?;
            host_state.state_json(&name, instance, &key).map_err(ui_error)
        })?)?;
        let host_state = state.clone();
        host.set("stateSet", Function::new(ctx.clone(), move |name: String, instance: String, key: String, json: String| {
            let instance = Uuid::parse_str(&instance).map_err(|error| ui_error(error.to_string()))?;
            host_state.set_state(&name, instance, &key, &json).map(|_| ()).map_err(ui_error)
        })?)?;
        let host_state = state.clone();
        host.set("stateDelete", Function::new(ctx.clone(), move |name: String, instance: String, key: String| {
            let instance = Uuid::parse_str(&instance).map_err(|error| ui_error(error.to_string()))?;
            host_state.delete_state(&name, instance, &key).map(|_| ()).map_err(ui_error)
        })?)?;
        let host_state = state.clone();
        host.set("update", Function::new(ctx.clone(), move |name: String, instance: String| {
            host_state.check_live().map_err(ui_error)?;
            let store = host_state.store.borrow();
            if let Some(handler) = &store.elements[ScriptUiState::element_index(
                &store, &name, Some(Uuid::parse_str(&instance).map_err(|error| ui_error(error.to_string()))?),
            ).map_err(ui_error)?].handlers[event_index(UiEventKind::StateUpdate)] {
                host_state.state_updates.borrow_mut().push(handler.registration);
            }
            Ok::<(), rquickjs::Error>(())
        })?)?;
        let measure_state = state.clone();
        let measure_session = session.clone();
        let measurements = state.publisher.subscribe();
        host.set("measure", Function::new(ctx.clone(), Async(move |ctx: Ctx<'js>, name: String, instance: String, kind: String, relative_to: String| {
            let connection = connection.clone();
            let state = measure_state.clone();
            let session = measure_session.clone();
            let revision = measurements.borrow().revision;
            async move {
                reject_if_stopped(&ctx, &session)?;
                let instance = Uuid::parse_str(&instance).map_err(|error| Exception::throw_message(&ctx, &error.to_string()))?;
                state.checked(&name, instance).map_err(|error| Exception::throw_message(&ctx, &error))?;
                let measured = connection.request(ScriptUiMeasureReq { session_id: state.session_id(), revision, element_id: name.clone(), instance_id: instance, relative_to })
                    .await
                    .map_err(|error| bridge_error(&ctx, error.to_string()))?;
                reject_if_stopped(&ctx, &session)?;
                state.checked(&name, instance).map_err(|error| Exception::throw_message(&ctx, &error))?;
                if !measured.width.is_finite() || measured.width < 0.0 || !measured.height.is_finite() || measured.height < 0.0 {
                    return Err(Exception::throw_message(&ctx, "invalid UI dimensions"));
                }
                Ok(match kind.as_str() {
                    "width" => measured.width.into_js(&ctx)?,
                    "height" => measured.height.into_js(&ctx)?,
                    "globalX" => match measured.global_x { Some(value) => value.into_js(&ctx)?, None => Value::new_null(ctx.clone()) },
                    "globalY" => match measured.global_y { Some(value) => value.into_js(&ctx)?, None => Value::new_null(ctx.clone()) },
                    _ => return Err(Exception::throw_message(&ctx, "invalid UI measurement")),
                })
            }
        }))?)?;
        let handler_state = state.clone();
        let handler_transfer = Rc::downgrade(&transfer);
        let handler_background = background.clone();
        let handler_session = session.clone();
        host.set("handlerRegister", Function::new(ctx.clone(), move |ctx: Ctx<'js>, name: String, instance: String, event: String, callback: Function<'js>| {
            reject_if_stopped(&ctx, &handler_session)?;
            let instance = Uuid::parse_str(&instance).map_err(|error| Exception::throw_message(&ctx, &error.to_string()))?;
            let event = parse_event(&event).map_err(|error| Exception::throw_message(&ctx, error.as_str()))?;
            handler_state.checked(&name, instance).map_err(|error| Exception::throw_message(&ctx, error.as_str()))?;
            let Some(handler_transfer) = handler_transfer.upgrade() else {
                return Err(Exception::throw_message(&ctx, "script session stopped"));
            };
            let descriptor = handler_transfer.capture(&ctx, callback)?;
            let target = BackgroundTarget::Ui { name: name.clone(), instance, event };
            handler_background.unregister(&target);
            let registration = handler_background.register(target.clone(), descriptor);
            if let Err(error) = handler_state.begin_handler_install(&name, instance, event, registration) {
                handler_background.unregister(&target);
                return Err(Exception::throw_message(&ctx, &error));
            }
            Ok(registration.to_string())
        })?)?;
        let handler_background_status = background.clone();
        host.set("handlerStatus", Function::new(ctx.clone(), move |registration: String| {
            let registration = Uuid::parse_str(&registration).ok();
            Ok::<Option<&'static str>, rquickjs::Error>(registration.and_then(|registration| match handler_background_status.status(registration) {
                Some(BackgroundRegistrationStatus::Pending) => Some("pending"),
                Some(BackgroundRegistrationStatus::Active) => Some("active"),
                Some(BackgroundRegistrationStatus::Failed) => Some("failed"),
                None => None,
            }))
        })?)?;
        let handler_state = state.clone();
        let handler_background = background.clone();
        let handler_session = session.clone();
        host.set("handlerClear", Function::new(ctx.clone(), move |ctx: Ctx<'js>, name: String, instance: String, event: String| {
            reject_if_stopped(&ctx, &handler_session)?;
            let instance = Uuid::parse_str(&instance).map_err(|error| Exception::throw_message(&ctx, &error.to_string()))?;
            let event = parse_event(&event).map_err(|error| Exception::throw_message(&ctx, error.as_str()))?;
            handler_state.clear_handler(&name, instance, event).map_err(|error| Exception::throw_message(&ctx, error.as_str()))?;
            handler_background.unregister(&BackgroundTarget::Ui { name, instance, event });
            Ok::<(), rquickjs::Error>(())
        })?)?;
        let factory: Function = ctx.eval(include_str!("ui.js"))?;
        let api: Object = factory.call((host, codec))?;
        Ok(Self { state, api: Persistent::save(ctx, api) })
    }

    pub(super) fn registry<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<Object<'js>> {
        self.api.clone().restore(ctx)?.get("registry")
    }

    pub(super) fn element<'js>(&self, ctx: &Ctx<'js>, name: &str, instance: Uuid) -> rquickjs::Result<Object<'js>> {
        self.state.checked(name, instance).map_err(ui_error)?;
        let api = self.api.clone().restore(ctx)?;
        let element: Function = api.get("element")?;
        element.call((name.to_owned(), instance.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(json: &str) -> String {
        format!(r#"{{"value":{json}}}"#)
    }

    #[test]
    fn two_runtime_ui_shared_store_and_states() {
        let publisher = ScriptUiPublisher::default();
        let snapshots = publisher.subscribe();
        let state = ScriptUiState::new(publisher, SessionControl::standalone());
        let main_instance = state.define("from_main", &envelope(r#"{"text":"main","states":{"count":1,"settings":{"enabled":false}}}"#)).unwrap();
        let background_instance = state.define("from_background", &envelope(r#"{"text":"background","states":{"count":2}}"#)).unwrap();
        assert_eq!(snapshots.borrow().elements.iter().map(|element| element.id.as_str()).collect::<Vec<_>>(), ["from_main", "from_background"]);
        let revision_before_state = snapshots.borrow().revision;
        state.set_field("from_background", background_instance, "text", &envelope(r#""written""#)).unwrap();
        state.set_state("from_main", main_instance, "count", &envelope("10")).unwrap();
        assert_eq!(snapshots.borrow().elements[1].text, "written");
        assert_eq!(snapshots.borrow().revision, revision_before_state + 1);
        assert_eq!(state.state_json("from_main", main_instance, "settings").unwrap(), Some(envelope(r#"{"enabled":false}"#)));
        state.set_state("from_main", main_instance, "settings", &envelope(r#"{"enabled":true}"#)).unwrap();
        assert_eq!(state.state_json("from_main", main_instance, "settings").unwrap(), Some(envelope(r#"{"enabled":true}"#)));
        assert_eq!(snapshots.borrow().revision, revision_before_state + 1);
    }

    #[test]
    fn two_runtime_ui_atomic_update_and_proxy_identity() {
        let publisher = ScriptUiPublisher::default();
        let snapshots = publisher.subscribe();
        let state = ScriptUiState::new(publisher, SessionControl::standalone());
        let original = state.define("same", &envelope(r#"{"text":"before"}"#)).unwrap();
        assert!(state.define("same", &envelope(r#"{"unknown":1}"#)).is_err());
        assert_eq!(state.lookup("same"), Some(original));
        let event = ScriptUiEvent {
            session_id: state.session_id(), element_id: "same".to_owned(), instance_id: original,
            events_version: snapshots.borrow().elements[0].events_version, event: ScriptUiEventKind::Click,
        };
        assert_eq!(state.installed_handler_registration(&event), None);
        let registration = Uuid::new_v4();
        state.begin_handler_install("same", original, UiEventKind::Pointer(ScriptUiEventKind::Click), registration).unwrap();
        let pending = snapshots.borrow().elements[0].events_version;
        assert!(!state.installed_handler_registration(&ScriptUiEvent { events_version: pending, ..event.clone() }).is_some());
        assert!(state.complete_handler_install("same", original, UiEventKind::Pointer(ScriptUiEventKind::Click), registration, true));
        let active = ScriptUiEvent { events_version: snapshots.borrow().elements[0].events_version, ..event.clone() };
        assert_eq!(state.installed_handler_registration(&active), Some(registration));
        assert_eq!(state.define("same", &envelope(r#"{"text":"after"}"#)).unwrap(), original);
        assert_eq!(state.installed_handler_registration(&active), Some(registration));
        assert!(state.record_json("same", original).unwrap().contains("after"));
    }

    #[test]
    fn ui_state_defaults_hidden_and_stacking_identity() {
        let publisher = ScriptUiPublisher::default();
        let snapshots = publisher.subscribe();
        let state = ScriptUiState::new(publisher, SessionControl::standalone());
        let first = state.define("a", &envelope("{}")).unwrap();
        assert!(!snapshots.borrow().elements[0].hidden);
        state.set_field("a", first, "hidden", &envelope("true")).unwrap();
        assert!(snapshots.borrow().elements[0].hidden);
        let second = state.define("b", &envelope("{}")).unwrap();
        assert_eq!(state.define("a", &envelope(r#"{"text":"updated"}"#)).unwrap(), first);
        assert!(snapshots.borrow().elements[0].hidden);
        assert_eq!(snapshots.borrow().elements.iter().map(|element| element.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(snapshots.borrow().elements[1].instance_id, second);
    }

    #[test]
    fn two_runtime_ui_proxy_and_state_identity() {
        use rquickjs::{Context, Runtime};

        let main_runtime = Runtime::new().unwrap();
        let main_context = Context::full(&main_runtime).unwrap();
        let background_runtime = Runtime::new().unwrap();
        let background_context = Context::full(&background_runtime).unwrap();
        let session = SessionControl::standalone();
        let publisher = ScriptUiPublisher::default();
        let state = Rc::new(ScriptUiState::new(publisher, session.clone()));
        let background = Rc::new(BackgroundRegistry::new(session.clone()));
        let main_transfer = main_context.with(|ctx| Rc::new(FunctionTransfer::new(&ctx).unwrap()));
        let background_transfer = background_context.with(|ctx| Rc::new(FunctionTransfer::new(&ctx).unwrap()));
        let main_bindings = main_context.with(|ctx| {
            let codec = super::super::value::codec(&ctx).unwrap();
            ScriptUiBindings::new(
                &ctx,
                codec,
                state.clone(),
                WsConnection::disconnected_for_test(),
                session.clone(),
                main_transfer.clone(),
                background.clone(),
            ).unwrap()
        });
        let background_bindings = background_context.with(|ctx| {
            let codec = super::super::value::codec(&ctx).unwrap();
            ScriptUiBindings::new(
                &ctx,
                codec,
                state.clone(),
                WsConnection::disconnected_for_test(),
                session.clone(),
                background_transfer.clone(),
                background.clone(),
            ).unwrap()
        });

        main_context.with(|ctx| {
            ctx.globals().set("ui", main_bindings.registry(&ctx).unwrap()).unwrap();
            ctx.eval::<(), _>(r#"
                ui('main', { text: "main", states: { nested: { enabled: false } } });
                globalThis.mainProxy = ui.main;
                mainProxy.states.nested = { enabled: true };
                if (ui.main !== mainProxy) throw new Error("main proxy identity changed");
            "#).unwrap();
        });
        background_context.with(|ctx| {
            ctx.globals().set("ui", background_bindings.registry(&ctx).unwrap()).unwrap();
            ctx.eval::<(), _>(r#"
                ui('background', { text: "background", states: { count: 2 } });
                ui.main.states.count = 10;
                globalThis.backgroundProxy = ui.background;
                if (ui.background !== backgroundProxy) throw new Error("background proxy identity changed");
            "#).unwrap();
        });
        assert_eq!(state.state_json("main", state.lookup("main").unwrap(), "nested"), Ok(Some(envelope(r#"{"enabled":true}"#))));
        assert_eq!(state.state_json("main", state.lookup("main").unwrap(), "count"), Ok(Some(envelope("10"))));
        assert_eq!(state.names(), vec!["main", "background"]);

        background_context.with(|ctx| {
            ctx.eval::<(), _>(r#"ui('main', { text: "updated" });"#).unwrap();
        });
        main_context.with(|ctx| {
            ctx.eval::<(), _>(r#"
                if (ui.main !== mainProxy || mainProxy.text !== "updated" || mainProxy.states.count !== 10)
                    throw new Error("update did not preserve shared proxy and states");
            "#).unwrap();
        });
        background_context.with(|ctx| {
            ctx.eval::<(), _>(r#"ui('main', null); ui('main', { text: "replacement", states: { count: 1 } });"#).unwrap();
        });
        main_context.with(|ctx| {
            ctx.eval::<(), _>(r#"
                let stale = mainProxy;
                let rejected = false;
                try { stale.text = "stale"; } catch (_) { rejected = true; }
                if (!rejected) throw new Error("stale visual proxy was accepted");
                if (ui.main === stale || ui.main.text !== "replacement") throw new Error("replacement identity mismatch");
                rejected = false;
                try { stale.states.count = 4; } catch (_) { rejected = true; }
                if (!rejected) throw new Error("stale state proxy was accepted");
            "#).unwrap();
        });

        drop(background_bindings);
        drop(main_bindings);
        drop(background_transfer);
        drop(main_transfer);
        drop(background);
        drop(state);
        drop(background_context);
        drop(background_runtime);
        drop(main_context);
        drop(main_runtime);
    }

    #[test]
    fn ui_handler_install_races_and_replacement_invalidate_stale_work() {
        let publisher = ScriptUiPublisher::default();
        let snapshots = publisher.subscribe();
        let state = ScriptUiState::new(publisher, SessionControl::standalone());
        let instance = state.define("button", &envelope("{}")).unwrap();
        let first = Uuid::new_v4();
        state.begin_handler_install("button", instance, UiEventKind::Pointer(ScriptUiEventKind::Click), first).unwrap();
        let pending_version = snapshots.borrow().elements[0].events_version;
        let event = ScriptUiEvent {
            session_id: state.session_id(), element_id: "button".to_owned(), instance_id: instance,
            events_version: pending_version, event: ScriptUiEventKind::Click,
        };
        assert_eq!(state.installed_handler_registration(&event), None);
        let second = Uuid::new_v4();
        state.begin_handler_install("button", instance, UiEventKind::Pointer(ScriptUiEventKind::Click), second).unwrap();
        assert!(!state.complete_handler_install("button", instance, UiEventKind::Pointer(ScriptUiEventKind::Click), first, true));
        assert_eq!(state.installed_handler_registration(&event), None);
        assert!(state.complete_handler_install("button", instance, UiEventKind::Pointer(ScriptUiEventKind::Click), second, true));
        let active = ScriptUiEvent { events_version: snapshots.borrow().elements[0].events_version, ..event.clone() };
        assert_eq!(state.installed_handler_registration(&active), Some(second));
        state.remove("button").unwrap();
        let replacement = state.define("button", &envelope("{}")).unwrap();
        assert!(!state.complete_handler_install("button", instance, UiEventKind::Pointer(ScriptUiEventKind::Click), second, true));
        assert_eq!(state.installed_handler_registration(&active), None);
        let third = Uuid::new_v4();
        state.begin_handler_install("button", replacement, UiEventKind::Pointer(ScriptUiEventKind::Click), third).unwrap();
        assert!(!state.complete_handler_install("button", replacement, UiEventKind::Pointer(ScriptUiEventKind::Click), second, true));
        assert!(state.complete_handler_install("button", replacement, UiEventKind::Pointer(ScriptUiEventKind::Click), third, false));
        let failed_version = snapshots.borrow().elements[0].events_version;
        assert_eq!(state.installed_handler_registration(&ScriptUiEvent { instance_id: replacement, events_version: failed_version, ..active }), None);
        let fourth = Uuid::new_v4();
        state.begin_handler_install("button", replacement, UiEventKind::Pointer(ScriptUiEventKind::Click), fourth).unwrap();
        assert_eq!(state.clear_handler("button", replacement, UiEventKind::Pointer(ScriptUiEventKind::Click)).unwrap(), Some(fourth));
        assert!(!state.complete_handler_install("button", replacement, UiEventKind::Pointer(ScriptUiEventKind::Click), fourth, true));
    }

    #[test]
    fn ui_callback_setter_retries_failed_function_without_exposing_properties() {
        use rquickjs::{Context, Runtime};

        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            let source = format!(r#"
                const records = new Map(), statuses = new Map(), registrations = [];
                let nextRegistration = 0;
                const host = {{
                    stopped: () => false,
                    lookup: name => records.has(name) ? records.get(name).instance : undefined,
                    names: () => [...records.keys()],
                    record: (name, instance) => JSON.stringify({{ value: records.get(name).instance === instance ? records.get(name) : (() => {{ throw new Error('stale'); }})() }}),
                    define: (name, json) => {{ const instance = 'instance-' + name; const definition = JSON.parse(json).value; records.set(name, {{ instance, values: definition, states: {{}}, eventsVersion: 1 }}); return instance; }},
                    remove: name => records.delete(name),
                    setField: (name, instance, field, json) => {{ const record = records.get(name); if (record.instance !== instance) throw new Error('stale'); if (field === 'states') record.states = JSON.parse(json).value; else record.values[field] = JSON.parse(json).value; }},
                    deleteField: (name, instance, field) => {{ const record = records.get(name); if (field === 'states') record.states = {{}}; else delete record.values[field]; }},
                    stateKeys: (name, instance) => Object.keys(records.get(name).states),
                    stateGet: (name, instance, key) => records.get(name).states[key] === undefined ? undefined : JSON.stringify({{ value: records.get(name).states[key] }}),
                    stateSet: (name, instance, key, json) => records.get(name).states[key] = JSON.parse(json).value,
                    stateDelete: (name, instance, key) => delete records.get(name).states[key],
                    measure: async () => 0,
                    handlerRegister: (name, instance, event, callback) => {{ const registration = 'registration-' + (++nextRegistration); registrations.push(registration); statuses.set(registration, 'pending'); return registration; }},
                    handlerStatus: registration => statuses.get(registration),
                    handlerClear: () => {{}},
                }};
                const ui = ({}) (host, {{ isNative: () => false, encode: value => JSON.stringify({{ value }}), decode: json => JSON.parse(json).value }}).registry;
                ui('button', {{}});
                const callback = function() {{}};
                if (ui.button.setOnClick(callback) !== ui.button) throw new Error('setter did not return the element');
                if (ui.button.setOnClick(callback) !== ui.button) throw new Error('pending retry did not return the element');
                statuses.set(registrations[0], 'failed');
                ui.button.setOnClick(callback);
                if (registrations.length !== 2 || registrations[0] === registrations[1]) throw new Error('failed registration did not retry');
                if ('onClick' in ui.button || Object.keys(ui.button).includes('onClick') || Object.getOwnPropertyDescriptor(ui.button, 'onClick') !== undefined) throw new Error('legacy callback property exposed');
                let rejected = false;
                try {{ ui.button.setOnClick(undefined); }} catch (_) {{ rejected = true; }}
                if (!rejected) throw new Error('undefined callback accepted');
                ui.button.setOnClick(null);
            "#, include_str!("ui.js"));
            ctx.eval::<(), _>(source).unwrap();
        });
    }
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod legacy_tests;
