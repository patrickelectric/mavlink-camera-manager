/// /References:
/// 1 - https://gitlab.freedesktop.org/slomo/rtp-rapid-sync-example
/// 2 - https://github.com/gtk-rs/gtk3-rs/blob/master/examples/list_box_model/row_data/imp.rs
mod imp {
    use std::sync::{Arc, Mutex};

    use once_cell::sync::Lazy;

    use gst::glib::{self, subclass::prelude::*, *};
    use gst_rtsp_server::subclass::prelude::*;

    // The actual data structure that stores our values. This is not accessible
    // directly from the outside.
    #[derive(Default)]
    pub struct Factory {
        element: Arc<Mutex<Option<gst::Element>>>,
    }

    // Basic declaration of our type for the GObject type system
    #[glib::object_subclass]
    impl ObjectSubclass for Factory {
        const NAME: &'static str = "MyRTSPMediaFactory";
        type Type = super::Factory;
        type ParentType = gst_rtsp_server::RTSPMediaFactory;
    }

    // The ObjectImpl trait provides the setters/getters for GObject properties.
    // Here we need to provide the values that are internally stored back to the
    // caller, or store whatever new value the caller is providing.
    //
    // This maps between the GObject properties and our internal storage of the
    // corresponding values of the properties.
    impl ObjectImpl for Factory {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
                vec![glib::ParamSpecObject::new(
                    "element",
                    "Element",
                    "Element",
                    gst::Element::static_type(),
                    glib::ParamFlags::READWRITE,
                )]
            });

            PROPERTIES.as_ref()
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "element" => {
                    let pipeline = value
                        .get()
                        .expect("type conformity checked by `Object::set_property`");
                    *self.element.lock().unwrap() = pipeline;
                }
                _ => unimplemented!(),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "element" => self.element.lock().unwrap().to_value(),
                _ => unimplemented!(),
            }
        }
    }

    impl RTSPMediaFactoryImpl for Factory {
        // Create the custom stream producer bin.
        fn create_element(&self, _url: &gst_rtsp::RTSPUrl) -> Option<gst::Element> {
            let element = self.element.lock().unwrap();

            if element.is_none() {
                // gst::gst_error!(CAT, obj: factory, "Error creating RTSP elements: {err}");
            };

            element.clone()
        }
    }
}

gst::glib::wrapper! {
    pub struct Factory(ObjectSubclass<imp::Factory>) @extends gst_rtsp_server::RTSPMediaFactory;
}

// Trivial constructor for the media factory.
impl Factory {
    pub fn new(element: &gst::Element) -> Self {
        gst::glib::Object::new::<Self>(&[("element", element)])
    }
}
