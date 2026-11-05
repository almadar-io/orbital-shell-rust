use blitz_shell::BlitzShellNetCallback;
use std::sync::Arc;

use blitz_dom::net::Resource;
use blitz_shell::BlitzShellEvent;
#[cfg(any(feature = "net", test))]
use blitz_traits::net::SharedCallback;
use blitz_traits::net::{NetCallback, NetProvider};
use winit::event_loop::EventLoopProxy;

pub struct DioxusNativeNetProvider {
    callback: Arc<dyn NetCallback<Resource> + 'static>,
    #[cfg(feature = "net")]
    inner_net_provider: Arc<dyn NetProvider<Resource> + 'static>,
}

#[cfg(any(feature = "net", test))]
fn image_request_callback(
    callback: SharedCallback<Resource>,
    node_id: usize,
    src: String,
) -> SharedCallback<Resource> {
    Arc::new(
        move |doc_id, result: Result<Resource, Option<String>>| match result {
            Ok(resource) => callback.call(doc_id, Ok(resource)),
            Err(_) => callback.call(doc_id, Ok(Resource::ImageError(node_id, src.clone()))),
        },
    )
}
impl DioxusNativeNetProvider {
    pub fn shared(proxy: EventLoopProxy<BlitzShellEvent>) -> Arc<dyn NetProvider<Resource>> {
        Arc::new(Self::new(proxy)) as Arc<dyn NetProvider<Resource>>
    }

    pub fn new(proxy: EventLoopProxy<BlitzShellEvent>) -> Self {
        let net_callback = BlitzShellNetCallback::shared(proxy);

        #[cfg(feature = "net")]
        let net_provider = blitz_net::Provider::shared(net_callback.clone());

        Self {
            callback: net_callback,
            #[cfg(feature = "net")]
            inner_net_provider: net_provider,
        }
    }
}

impl NetProvider<Resource> for DioxusNativeNetProvider {
    fn fetch(
        &self,
        doc_id: usize,
        request: blitz_traits::net::Request,
        handler: blitz_traits::net::BoxedHandler<Resource>,
    ) {
        let error_resource = handler.error_resource();
        if request.url.scheme() == "dioxus" {
            #[allow(clippy::single_match)]
            // cfg'd code has multiple branches in some configurations
            match dioxus_asset_resolver::native::serve_asset(request.url.path()) {
                Ok(res) => {
                    #[cfg(feature = "tracing")]
                    tracing::trace!("fetching asset from file system success {request:#?}");
                    handler.bytes(doc_id, res.into_body().into(), self.callback.clone())
                }
                Err(_) => {
                    #[cfg(feature = "tracing")]
                    tracing::warn!("fetching asset from file system error {request:#?}");
                    if let Some(error) = error_resource {
                        self.callback.call(doc_id, Ok(error));
                    }
                }
            }
        } else {
            #[cfg(feature = "net")]
            if let Some(Resource::ImageError(node_id, src)) = error_resource {
                let request_callback = image_request_callback(self.callback.clone(), node_id, src);
                blitz_net::Provider::shared(request_callback).fetch(doc_id, request, handler);
            } else {
                self.inner_net_provider.fetch(doc_id, request, handler);
            }

            #[cfg(not(feature = "net"))]
            if let Some(error) = error_resource {
                self.callback.call(doc_id, Ok(error));
            }

            #[cfg(all(not(feature = "net"), feature = "tracing"))]
            tracing::warn!("net feature not enabled, cannot fetch {request:#?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn image_fetch_error_keeps_request_node_and_success_passes_through() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let output = seen.clone();
        let callback: SharedCallback<Resource> = Arc::new(move |doc_id, result| {
            output.lock().unwrap().push((doc_id, result));
        });
        let request_callback = image_request_callback(callback, 42, "broken.png".into());

        request_callback.call(7, Err(Some("not found".into())));
        request_callback.call(7, Ok(Resource::None));

        let events = seen.lock().unwrap();
        assert!(
            matches!(&events[0], (7, Ok(Resource::ImageError(42, src))) if src == "broken.png")
        );
        assert!(matches!(&events[1], (7, Ok(Resource::None))));
    }
}
