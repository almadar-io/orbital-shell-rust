use orbital_core::{Value, ValueMap};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use crate::renderer::EventSink;
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use dioxus::prelude::*;
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use orbital_core::client::helpers::{get_number, get_string, merge_class_name};
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use std::cell::RefCell;
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use std::collections::{HashMap, HashSet};
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use std::rc::Rc;
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Debug, PartialEq)]
pub enum VideoSource {
    YouTube(String),
    Page(String),
}

impl VideoSource {
    pub fn from_props(props: &Value) -> Option<Self> {
        let object = props.as_object()?;
        match object.get("videoId") {
            Some(Value::String(id)) if !id.is_empty() => {
                return Some(Self::YouTube(id.to_string()))
            }
            _ => {}
        }
        match object.get("src") {
            Some(Value::String(url)) if !url.is_empty() => Some(Self::Page(url.to_string())),
            _ => None,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::YouTube(id) | Self::Page(id) => id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VideoOverlay {
    pub node_id: usize,
    pub source: VideoSource,
    pub title: String,
    pub autoplay: bool,
    pub controls: bool,
    pub start_seconds: f64,
    pub bounds: VideoBounds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoScriptBridge {
    Android,
    Ios,
    Desktop,
}

pub fn youtube_page(overlay: &VideoOverlay, bridge: VideoScriptBridge) -> Option<String> {
    let VideoSource::YouTube(id) = &overlay.source else {
        return None;
    };
    let id = serde_json::to_string(id).ok()?.replace('<', "\\u003c");
    let (ready, playing, failed) = match bridge {
        VideoScriptBridge::Android => (
            "OrbitalVideo.ready()".to_string(),
            "OrbitalVideo.play()".to_string(),
            "OrbitalVideo.error(String(code))".to_string(),
        ),
        VideoScriptBridge::Ios => (
            format!("window.webkit.messageHandlers.orbitalVideo.postMessage({{node:{},kind:0}})", overlay.node_id),
            format!("window.webkit.messageHandlers.orbitalVideo.postMessage({{node:{},kind:1}})", overlay.node_id),
            format!("window.webkit.messageHandlers.orbitalVideo.postMessage({{node:{},kind:2,code:String(code)}})", overlay.node_id),
        ),
        VideoScriptBridge::Desktop => (
            format!("window.ipc.postMessage(JSON.stringify({{node:{},kind:0}}))", overlay.node_id),
            format!("window.ipc.postMessage(JSON.stringify({{node:{},kind:1}}))", overlay.node_id),
            format!("window.ipc.postMessage(JSON.stringify({{node:{},kind:2,code:String(code)}}))", overlay.node_id),
        ),
    };
    Some(format!(
        "<!doctype html><meta name='viewport' content='width=device-width,initial-scale=1'>\
         <style>html,body,#player{{margin:0;width:100%;height:100%;background:black}}</style>\
         <div id='player'></div><script>function orbitalApiFailed(){{var code='api_unavailable';{failed}}}</script>\
         <script src='https://www.youtube.com/iframe_api' onerror='orbitalApiFailed()'></script><script>\
         function onYouTubeIframeAPIReady(){{new YT.Player('player',{{videoId:{id},\
         playerVars:{{autoplay:{},controls:{},start:{},rel:0}},events:{{\
         onReady:function(){{{ready}}},\
         onStateChange:function(e){{if(e.data===1){{{playing}}}}},\
         onError:function(e){{var code=e.data;{failed}}}}}}})}}\
         </script>",
        u8::from(overlay.autoplay),
        u8::from(overlay.controls),
        overlay.start_seconds.max(0.0).floor() as u64,
    ))
}

#[derive(Clone, Debug, PartialEq)]
pub enum VideoOverlayEvent {
    Ready {
        source: String,
    },
    Playing {
        source: String,
    },
    Failed {
        source: String,
        error: String,
        code: String,
    },
}

impl VideoOverlayEvent {
    pub fn authored_dispatch(&self, props: &Value) -> Option<(String, Value)> {
        let object = props.as_object()?;
        let (event_prop, payload) = match self {
            Self::Ready { source } => {
                let mut payload = ValueMap::new();
                payload.insert("source".into(), Value::String(source.clone().into()));
                ("readyEvent", payload)
            }
            Self::Playing { source } => {
                let mut payload = ValueMap::new();
                payload.insert("source".into(), Value::String(source.clone().into()));
                ("playEvent", payload)
            }
            Self::Failed { error, code, .. } => {
                let mut payload = ValueMap::new();
                let message_prop = match code.as_str() {
                    "api_unavailable" => "apiUnavailableMessage",
                    "2" => "invalidVideoMessage",
                    "100" => "notFoundMessage",
                    "101" | "150" => "notEmbeddableMessage",
                    _ => "playbackFailedMessage",
                };
                let message = object
                    .get(message_prop)
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(error);
                payload.insert("error".into(), Value::String(message.to_string().into()));
                payload.insert("code".into(), Value::String(code.clone().into()));
                ("errorEvent", payload)
            }
        };
        let Some(Value::String(name)) = object.get(event_prop) else {
            return None;
        };
        if name.is_empty() {
            return None;
        }
        Some((name.to_string(), Value::Object(payload.into())))
    }
}

static EVENTS: OnceLock<Mutex<VecDeque<(usize, VideoOverlayEvent)>>> = OnceLock::new();

pub fn queue_event(node_id: usize, event: VideoOverlayEvent) {
    EVENTS
        .get_or_init(Default::default)
        .lock()
        .expect("video event queue poisoned")
        .push_back((node_id, event));
    #[cfg(all(feature = "native", not(target_arch = "wasm32")))]
    dioxus_native::wake_layout();
}

pub fn drain_events() -> Vec<(usize, VideoOverlayEvent)> {
    let mut queue = EVENTS
        .get_or_init(Default::default)
        .lock()
        .expect("video event queue poisoned");
    queue.drain(..).collect()
}

#[cfg(target_os = "android")]
static ANDROID_CLASS: OnceLock<(jni::JavaVM, jni::objects::GlobalRef)> = OnceLock::new();

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_google_androidgamesdk_OrbitalVideoInput_nativeInstall(
    mut env: jni::JNIEnv<'_>,
    class: jni::objects::JClass<'_>,
) {
    let installed = env
        .get_java_vm()
        .and_then(|vm| env.new_global_ref(class).map(|reference| (vm, reference)));
    match installed {
        Ok(value) => {
            let _ = ANDROID_CLASS.set(value);
        }
        Err(error) => tracing::error!(%error, "video overlay JNI installation failed"),
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "system" fn Java_com_google_androidgamesdk_OrbitalVideoInput_nativeVideoEvent(
    mut env: jni::JNIEnv<'_>,
    _class: jni::objects::JClass<'_>,
    node_id: jni::sys::jint,
    kind: jni::sys::jint,
    source: jni::objects::JString<'_>,
    error: jni::objects::JString<'_>,
    code: jni::objects::JString<'_>,
) {
    let Ok(source) = env.get_string(&source).map(|value| value.into()) else {
        return;
    };
    let event = match kind {
        0 => VideoOverlayEvent::Ready { source },
        1 => VideoOverlayEvent::Playing { source },
        2 => {
            let Ok(error) = env.get_string(&error).map(|value| value.into()) else {
                return;
            };
            let Ok(code) = env.get_string(&code).map(|value| value.into()) else {
                return;
            };
            VideoOverlayEvent::Failed {
                source,
                error,
                code,
            }
        }
        _ => return,
    };
    if let Ok(node_id) = usize::try_from(node_id) {
        queue_event(node_id, event);
    }
}

#[cfg(target_os = "android")]
pub fn upsert_android(overlay: &VideoOverlay) -> Result<(), String> {
    use jni::objects::{JObject, JValue};
    let (vm, class) = ANDROID_CLASS
        .get()
        .ok_or("video overlay JNI not installed")?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|error| error.to_string())?;
    let node_id = i32::try_from(overlay.node_id).map_err(|error| error.to_string())?;
    let (id, url) = match &overlay.source {
        VideoSource::YouTube(id) => (id.as_str(), ""),
        VideoSource::Page(url) => ("", url.as_str()),
    };
    let id = JObject::from(env.new_string(id).map_err(|error| error.to_string())?);
    let url = JObject::from(env.new_string(url).map_err(|error| error.to_string())?);
    let title = JObject::from(
        env.new_string(&overlay.title)
            .map_err(|error| error.to_string())?,
    );
    let html = youtube_page(overlay, VideoScriptBridge::Android).unwrap_or_default();
    let html = JObject::from(env.new_string(html).map_err(|error| error.to_string())?);
    env.call_static_method(
        class,
        "upsert",
        "(ILjava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZZDFFFF)V",
        &[
            JValue::Int(node_id),
            JValue::Object(&id),
            JValue::Object(&url),
            JValue::Object(&title),
            JValue::Object(&html),
            JValue::Bool(overlay.autoplay.into()),
            JValue::Bool(overlay.controls.into()),
            JValue::Double(overlay.start_seconds),
            JValue::Float(overlay.bounds.x),
            JValue::Float(overlay.bounds.y),
            JValue::Float(overlay.bounds.width),
            JValue::Float(overlay.bounds.height),
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(target_os = "android")]
pub fn remove_android(node_id: usize) -> Result<(), String> {
    use jni::objects::JValue;
    let (vm, class) = ANDROID_CLASS
        .get()
        .ok_or("video overlay JNI not installed")?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|error| error.to_string())?;
    let node_id = i32::try_from(node_id).map_err(|error| error.to_string())?;
    env.call_static_method(class, "remove", "(I)V", &[JValue::Int(node_id)])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(target_os = "ios")]
extern "C" {
    fn orbital_video_overlay_upsert(
        node_id: i32,
        video_id: *const std::ffi::c_char,
        url: *const std::ffi::c_char,
        title: *const std::ffi::c_char,
        html: *const std::ffi::c_char,
        autoplay: bool,
        controls: bool,
        start: f64,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    );
    fn orbital_video_overlay_remove(node_id: i32);
}

#[cfg(target_os = "ios")]
#[no_mangle]
pub extern "C" fn orbital_native_video_event(
    node_id: i32,
    kind: i32,
    source: *const std::ffi::c_char,
    error: *const std::ffi::c_char,
    code: *const std::ffi::c_char,
) {
    use std::ffi::CStr;
    if node_id < 0 || source.is_null() {
        return;
    }
    let source = unsafe { CStr::from_ptr(source) }
        .to_string_lossy()
        .into_owned();
    let event = match kind {
        0 => VideoOverlayEvent::Ready { source },
        1 => VideoOverlayEvent::Playing { source },
        2 if !error.is_null() && !code.is_null() => VideoOverlayEvent::Failed {
            source,
            error: unsafe { CStr::from_ptr(error) }
                .to_string_lossy()
                .into_owned(),
            code: unsafe { CStr::from_ptr(code) }
                .to_string_lossy()
                .into_owned(),
        },
        _ => return,
    };
    queue_event(node_id as usize, event);
}

#[cfg(target_os = "ios")]
pub fn upsert_ios(overlay: &VideoOverlay) -> Result<(), String> {
    use std::ffi::CString;
    let node_id = i32::try_from(overlay.node_id).map_err(|error| error.to_string())?;
    let (id, url) = match &overlay.source {
        VideoSource::YouTube(id) => (id.as_str(), ""),
        VideoSource::Page(url) => ("", url.as_str()),
    };
    let id = CString::new(id).map_err(|error| error.to_string())?;
    let url = CString::new(url).map_err(|error| error.to_string())?;
    let title = CString::new(overlay.title.as_str()).map_err(|error| error.to_string())?;
    let html = youtube_page(overlay, VideoScriptBridge::Ios).unwrap_or_default();
    let html = CString::new(html).map_err(|error| error.to_string())?;
    unsafe {
        orbital_video_overlay_upsert(
            node_id,
            id.as_ptr(),
            url.as_ptr(),
            title.as_ptr(),
            html.as_ptr(),
            overlay.autoplay,
            overlay.controls,
            overlay.start_seconds,
            overlay.bounds.x,
            overlay.bounds.y,
            overlay.bounds.width,
            overlay.bounds.height,
        );
    }
    Ok(())
}

#[cfg(target_os = "ios")]
pub fn remove_ios(node_id: usize) -> Result<(), String> {
    let node_id = i32::try_from(node_id).map_err(|error| error.to_string())?;
    unsafe { orbital_video_overlay_remove(node_id) };
    Ok(())
}

#[cfg(all(feature = "research-video", target_os = "macos"))]
struct DesktopVideoView {
    view: wry::WebView,
    overlay: VideoOverlay,
}

#[cfg(all(feature = "research-video", target_os = "macos"))]
thread_local! {
    static DESKTOP_VIEWS: RefCell<HashMap<usize, DesktopVideoView>> = RefCell::new(HashMap::new());
}

#[cfg(all(feature = "research-video", target_os = "macos"))]
fn desktop_rect(bounds: VideoBounds) -> wry::Rect {
    wry::Rect {
        position: dioxus_native::PhysicalPosition::new(
            bounds.x.round() as i32,
            bounds.y.round() as i32,
        )
        .into(),
        size: dioxus_native::PhysicalSize::new(
            bounds.width.max(1.0).round() as u32,
            bounds.height.max(1.0).round() as u32,
        )
        .into(),
    }
}

#[cfg(all(feature = "research-video", target_os = "macos"))]
fn upsert_desktop(window: &dioxus_native::Window, overlay: &VideoOverlay) -> Result<(), String> {
    use wry::{PageLoadEvent, WebViewBuilder};
    DESKTOP_VIEWS.with(|views| {
        let mut views = views.borrow_mut();
        if let Some(existing) = views.get_mut(&overlay.node_id) {
            if existing.overlay.source == overlay.source
                && existing.overlay.autoplay == overlay.autoplay
                && existing.overlay.controls == overlay.controls
                && existing.overlay.start_seconds == overlay.start_seconds
            {
                existing
                    .view
                    .set_bounds(desktop_rect(overlay.bounds))
                    .map_err(|error| error.to_string())?;
                existing.overlay = overlay.clone();
                return Ok(());
            }
        }
        views.remove(&overlay.node_id);
        let node_id = overlay.node_id;
        let source = overlay.source.name().to_string();
        let builder = WebViewBuilder::new()
            .with_bounds(desktop_rect(overlay.bounds))
            .with_autoplay(overlay.autoplay)
            .with_ipc_handler({
                let source = source.clone();
                move |request| {
                    let Ok(message) = serde_json::from_str::<DesktopVideoMessage>(request.body())
                    else {
                        return;
                    };
                    if message.node != node_id {
                        return;
                    }
                    let event = match message.kind {
                        0 => VideoOverlayEvent::Ready {
                            source: source.clone(),
                        },
                        1 => VideoOverlayEvent::Playing {
                            source: source.clone(),
                        },
                        2 => VideoOverlayEvent::Failed {
                            source: source.clone(),
                            error: "Video playback failed".into(),
                            code: message.code.unwrap_or_default(),
                        },
                        _ => return,
                    };
                    queue_event(node_id, event);
                }
            });
        let builder = match &overlay.source {
            VideoSource::YouTube(_) => builder.with_html(
                youtube_page(overlay, VideoScriptBridge::Desktop)
                    .ok_or("video page generation failed")?,
            ),
            VideoSource::Page(url) => {
                builder
                    .with_url(url.clone())
                    .with_on_page_load_handler(move |event, _| {
                        if matches!(event, PageLoadEvent::Finished) {
                            queue_event(
                                node_id,
                                VideoOverlayEvent::Ready {
                                    source: source.clone(),
                                },
                            );
                        }
                    })
            }
        };
        let view = builder
            .build_as_child(window)
            .map_err(|error| error.to_string())?;
        views.insert(
            node_id,
            DesktopVideoView {
                view,
                overlay: overlay.clone(),
            },
        );
        Ok(())
    })
}

#[cfg(all(feature = "research-video", target_os = "macos"))]
#[derive(serde::Deserialize)]
struct DesktopVideoMessage {
    node: usize,
    kind: u8,
    code: Option<String>,
}

#[cfg(all(feature = "research-video", target_os = "macos"))]
fn remove_desktop(node_id: usize) {
    DESKTOP_VIEWS.with(|views| {
        views.borrow_mut().remove(&node_id);
    });
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
#[derive(Clone)]
struct VideoEntry {
    props: Value,
    sink: Option<EventSink>,
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
thread_local! {
    static VIDEO_ENTRIES: RefCell<HashMap<usize, VideoEntry>> = RefCell::new(HashMap::new());
    static VISIBLE_VIDEO: RefCell<HashMap<usize, VideoOverlay>> = RefCell::new(HashMap::new());
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
static NEXT_VIDEO_ID: AtomicUsize = AtomicUsize::new(1);

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
#[derive(Clone)]
struct VideoMount(Rc<VideoMountInner>);

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
struct VideoMountInner(usize);

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
impl Drop for VideoMountInner {
    fn drop(&mut self) {
        VIDEO_ENTRIES.with(|entries| entries.borrow_mut().remove(&self.0));
    }
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
#[derive(Props, Clone, PartialEq)]
pub struct VideoAnchorProps {
    pub props: Value,
    pub sink: Option<EventSink>,
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
#[allow(non_snake_case)]
pub fn VideoAnchor(anchor: VideoAnchorProps) -> Element {
    let mount = use_hook(|| {
        VideoMount(Rc::new(VideoMountInner(
            NEXT_VIDEO_ID.fetch_add(1, Ordering::Relaxed),
        )))
    });
    VIDEO_ENTRIES.with(|entries| {
        entries.borrow_mut().insert(
            mount.0 .0,
            VideoEntry {
                props: anchor.props.clone(),
                sink: anchor.sink,
            },
        );
    });
    let id = mount.0 .0.to_string();
    let props = &anchor.props;
    let source = VideoSource::from_props(props);
    let class = merge_class_name(
        "aspect-video w-full overflow-hidden rounded-md bg-muted",
        props,
    );
    let title = get_string(props, "title")
        .filter(|value| !value.is_empty())
        .or_else(|| get_string(props, "aria-label"));
    let no_video = get_string(props, "noVideoLabel").unwrap_or_default();
    let role = get_string(props, "role").or_else(|| {
        matches!(source.as_ref(), Some(VideoSource::YouTube(_))).then(|| "region".into())
    });
    let labelled_by = get_string(props, "aria-labelledby");
    let described_by = get_string(props, "aria-describedby");
    let live = get_string(props, "aria-live");
    let current = get_string(props, "aria-current");
    let pressed = get_string(props, "aria-pressed");
    let expanded = props
        .as_object()
        .and_then(|object| object.get("aria-expanded"))
        .and_then(Value::as_bool)
        .map(|value| value.to_string());
    let selected = props
        .as_object()
        .and_then(|object| object.get("aria-selected"))
        .and_then(Value::as_bool)
        .map(|value| value.to_string());
    let hidden = props
        .as_object()
        .and_then(|object| object.get("aria-hidden"))
        .and_then(Value::as_bool)
        .map(|value| value.to_string());
    let busy = props
        .as_object()
        .and_then(|object| object.get("aria-busy"))
        .and_then(Value::as_bool)
        .map(|value| value.to_string());
    let tab_index = get_number(props, "tabIndex").map(|value| (value as i64).to_string());
    let lang = get_string(props, "lang");
    let dir = get_string(props, "dir");
    rsx! {
        div {
            "data-orbital-video": "{id}",
            class: "{class}",
            style: "display:block;width:100%;aspect-ratio:16 / 9",
            aria_label: title,
            aria_labelledby: labelled_by,
            aria_describedby: described_by,
            aria_live: live,
            aria_current: current,
            aria_pressed: pressed,
            aria_expanded: expanded,
            aria_selected: selected,
            aria_hidden: hidden,
            aria_busy: busy,
            tabindex: tab_index,
            role: role,
            lang: lang,
            dir: dir,
            if source.is_none() { span { class: "text-sm text-muted-foreground", "{no_video}" } }
        }
    }
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
fn overlay_from_props(node_id: usize, props: &Value, bounds: VideoBounds) -> Option<VideoOverlay> {
    let source = VideoSource::from_props(props)?;
    let object = props.as_object()?;
    let title = object
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let autoplay = matches!(object.get("autoplay"), Some(Value::Bool(true)));
    let controls = !matches!(object.get("controls"), Some(Value::Bool(false)));
    let start_seconds = match object.get("start") {
        Some(Value::Number(value)) => *value,
        _ => 0.0,
    };
    Some(VideoOverlay {
        node_id,
        source,
        title,
        autoplay,
        controls,
        start_seconds,
        bounds,
    })
}

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub fn sync_layout(window: &dioxus_native::Window, positions: &[(usize, [f32; 4])]) {
    let mut seen = HashSet::new();
    let mut updates = Vec::new();
    for &(id, [x, y, width, height]) in positions {
        let entry = VIDEO_ENTRIES.with(|entries| entries.borrow().get(&id).cloned());
        let Some(entry) = entry else { continue };
        let bounds = VideoBounds {
            x,
            y,
            width,
            height,
        };
        let Some(overlay) = overlay_from_props(id, &entry.props, bounds) else {
            continue;
        };
        seen.insert(id);
        let changed = VISIBLE_VIDEO.with(|visible| visible.borrow().get(&id) != Some(&overlay));
        if changed {
            updates.push(overlay);
        }
    }
    for overlay in updates {
        let id = overlay.node_id;
        #[cfg(target_os = "android")]
        let result = upsert_android(&overlay);
        #[cfg(target_os = "ios")]
        let result = upsert_ios(&overlay);
        #[cfg(all(feature = "research-video", target_os = "macos"))]
        let result = upsert_desktop(window, &overlay);
        #[cfg(not(any(target_os = "android", target_os = "ios", target_os = "macos")))]
        let result: Result<(), String> = Err("video overlay host is unavailable".into());
        #[cfg(all(target_os = "macos", not(feature = "research-video")))]
        let result: Result<(), String> = Err("video overlay research feature is disabled".into());
        #[cfg(any(target_os = "android", target_os = "ios"))]
        let _ = window;
        match result {
            Ok(()) => VISIBLE_VIDEO.with(|visible| {
                visible.borrow_mut().insert(id, overlay);
            }),
            Err(error) => tracing::error!(%error, "video overlay update failed"),
        }
    }
    VISIBLE_VIDEO.with(|visible| {
        let mut visible = visible.borrow_mut();
        for id in visible
            .keys()
            .filter(|id| !seen.contains(id))
            .copied()
            .collect::<Vec<_>>()
        {
            #[cfg(target_os = "android")]
            if let Err(error) = remove_android(id) {
                tracing::error!(%error, "Android video overlay removal failed");
            }
            #[cfg(target_os = "ios")]
            if let Err(error) = remove_ios(id) {
                tracing::error!(%error, "iOS video overlay removal failed");
            }
            #[cfg(all(feature = "research-video", target_os = "macos"))]
            remove_desktop(id);
            visible.remove(&id);
        }
    });
    for (id, event) in drain_events() {
        let entry = VIDEO_ENTRIES.with(|entries| entries.borrow().get(&id).cloned());
        let Some(entry) = entry else { continue };
        if let (Some(sink), Some(dispatch)) = (entry.sink, event.authored_dispatch(&entry.props)) {
            let source = match &event {
                VideoOverlayEvent::Ready { source }
                | VideoOverlayEvent::Playing { source }
                | VideoOverlayEvent::Failed { source, .. } => source,
            };
            if VideoSource::from_props(&entry.props)
                .as_ref()
                .map(VideoSource::name)
                == Some(source.as_str())
            {
                sink.call(dispatch);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_source_prefers_authored_id_and_events_keep_declared_payloads() {
        let props = Value::Object(
            [
                ("videoId".into(), Value::String("abc".into())),
                (
                    "src".into(),
                    Value::String("https://example.com/player".into()),
                ),
                ("readyEvent".into(), Value::String("READY".into())),
                ("playEvent".into(), Value::String("PLAY".into())),
                ("errorEvent".into(), Value::String("ERROR".into())),
            ]
            .into_iter()
            .collect(),
        );
        assert_eq!(
            VideoSource::from_props(&props),
            Some(VideoSource::YouTube("abc".into()))
        );
        assert_eq!(
            VideoOverlayEvent::Ready {
                source: "abc".into()
            }
            .authored_dispatch(&props),
            Some((
                "READY".into(),
                Value::Object(
                    [("source".into(), Value::String("abc".into()))]
                        .into_iter()
                        .collect()
                )
            ))
        );
        assert_eq!(
            VideoOverlayEvent::Playing {
                source: "abc".into()
            }
            .authored_dispatch(&props)
            .map(|v| v.0),
            Some("PLAY".into())
        );
        assert_eq!(
            VideoOverlayEvent::Failed {
                source: "abc".into(),
                error: "failed".into(),
                code: "5".into()
            }
            .authored_dispatch(&props)
            .map(|v| v.0),
            Some("ERROR".into())
        );
    }

    #[test]
    fn native_video_events_keep_node_order_until_the_ui_lane_drains_them() {
        let _ = drain_events();
        queue_event(4, VideoOverlayEvent::Ready { source: "a".into() });
        queue_event(2, VideoOverlayEvent::Playing { source: "b".into() });
        let events = drain_events();
        assert_eq!(events[0].0, 4);
        assert_eq!(events[1].0, 2);
        assert!(drain_events().is_empty());
    }

    #[test]
    fn youtube_page_escapes_id_and_uses_one_playback_contract_for_hosts() {
        let overlay = VideoOverlay {
            node_id: 7,
            source: VideoSource::YouTube("</script>".into()),
            title: "Lesson".into(),
            autoplay: true,
            controls: false,
            start_seconds: 12.0,
            bounds: VideoBounds {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 50.0,
            },
        };
        for bridge in [
            VideoScriptBridge::Android,
            VideoScriptBridge::Ios,
            VideoScriptBridge::Desktop,
        ] {
            let page = youtube_page(&overlay, bridge).unwrap();
            assert!(page.contains("\\u003c/script>"));
            assert!(page.contains("autoplay:1,controls:0,start:12"));
            assert!(page.contains("onerror='orbitalApiFailed()'"));
            assert!(!page.contains("videoId:\"</script>\""));
        }
    }

    #[test]
    fn native_video_errors_use_authored_messages_for_youtube_codes() {
        let props = Value::Object(
            [
                ("errorEvent".into(), Value::String("ERROR".into())),
                (
                    "notEmbeddableMessage".into(),
                    Value::String("Embedding blocked".into()),
                ),
                (
                    "apiUnavailableMessage".into(),
                    Value::String("Player unavailable".into()),
                ),
            ]
            .into_iter()
            .collect(),
        );
        for (code, message) in [
            ("101", "Embedding blocked"),
            ("150", "Embedding blocked"),
            ("api_unavailable", "Player unavailable"),
        ] {
            let (_, payload) = VideoOverlayEvent::Failed {
                source: "abc".into(),
                error: "fallback".into(),
                code: code.into(),
            }
            .authored_dispatch(&props)
            .unwrap();
            assert_eq!(
                payload
                    .as_object()
                    .and_then(|object| object.get("error"))
                    .and_then(Value::as_str),
                Some(message)
            );
        }
    }
}
