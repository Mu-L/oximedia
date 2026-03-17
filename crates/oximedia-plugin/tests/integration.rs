//! Integration test: full plugin lifecycle (register → lookup → use → unregister).
//!
//! Tests the end-to-end workflow that a host application would follow:
//! 1. Create a `PluginRegistry`.
//! 2. Register one or more plugins.
//! 3. Look up codecs and verify availability.
//! 4. Attempt to create decoders/encoders.
//! 5. Unregister all plugins and verify the registry is empty.

use oximedia_codec::{CodecError, EncoderConfig};
use oximedia_plugin::{
    CodecPluginInfo, PluginCapability, PluginRegistry, StaticPlugin, PLUGIN_API_VERSION,
};
use std::collections::HashMap;
use std::sync::Arc;

fn make_plugin(name: &str, codecs: &[(&str, bool, bool)]) -> Arc<dyn oximedia_plugin::CodecPlugin> {
    let info = CodecPluginInfo {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        author: "Integration Test".to_string(),
        description: format!("Integration test plugin: {name}"),
        api_version: PLUGIN_API_VERSION,
        license: "MIT".to_string(),
        patent_encumbered: false,
    };

    let mut plugin = StaticPlugin::new(info);
    for (codec, decode, encode) in codecs {
        plugin = plugin.add_capability(PluginCapability {
            codec_name: (*codec).to_string(),
            can_decode: *decode,
            can_encode: *encode,
            pixel_formats: vec!["yuv420p".to_string()],
            properties: HashMap::new(),
        });
    }
    Arc::new(plugin)
}

/// Full lifecycle: register → lookup → create → clear.
#[test]
fn test_full_plugin_lifecycle() {
    // 1. Create registry.
    let registry = PluginRegistry::empty();
    assert_eq!(registry.plugin_count(), 0);

    // 2. Register plugins.
    let av1_plugin = make_plugin("av1-plugin", &[("av1", true, true)]);
    let vp9_plugin = make_plugin("vp9-plugin", &[("vp9", true, true), ("vp8", true, false)]);
    registry.register(av1_plugin).expect("register av1");
    registry.register(vp9_plugin).expect("register vp9");
    assert_eq!(registry.plugin_count(), 2);

    // 3. Verify codec availability.
    assert!(registry.has_codec("av1"));
    assert!(registry.has_codec("vp9"));
    assert!(registry.has_codec("vp8"));
    assert!(!registry.has_codec("h264"));

    assert!(registry.has_decoder("av1"));
    assert!(registry.has_encoder("av1"));
    assert!(registry.has_decoder("vp8"));
    assert!(!registry.has_encoder("vp8")); // vp8 is decode-only

    // 4. Plugin listing.
    let plugins = registry.list_plugins();
    assert_eq!(plugins.len(), 2);
    let names: Vec<&str> = plugins.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"av1-plugin"));
    assert!(names.contains(&"vp9-plugin"));

    // 5. Codec listing.
    let codecs = registry.list_codecs();
    assert_eq!(codecs.len(), 3);

    // 6. find_plugin_for_codec returns correct plugin.
    let av1_info = registry.find_plugin_for_codec("av1").expect("find av1");
    assert_eq!(av1_info.name, "av1-plugin");

    let vp8_info = registry.find_plugin_for_codec("vp8").expect("find vp8");
    assert_eq!(vp8_info.name, "vp9-plugin");

    // 7. Create decoder (fails because StaticPlugin has no decoder factory).
    let decode_result = registry.find_decoder("av1");
    assert!(decode_result.is_err()); // No factory registered, expected

    // 8. Create encoder (fails for same reason).
    let encode_result = registry.find_encoder("av1", EncoderConfig::default());
    assert!(encode_result.is_err());

    // 9. Unregister all plugins.
    registry.clear();
    assert_eq!(registry.plugin_count(), 0);
    assert!(!registry.has_codec("av1"));
    assert!(registry.list_plugins().is_empty());
    assert!(registry.list_codecs().is_empty());
}

/// Duplicate plugin registration is rejected.
#[test]
fn test_duplicate_registration_rejected() {
    let registry = PluginRegistry::empty();
    let p1 = make_plugin("my-plugin", &[("h264", true, false)]);
    let p2 = make_plugin("my-plugin", &[("h265", true, false)]);
    registry.register(p1).expect("first registration");
    let err = registry.register(p2).expect_err("second should fail");
    assert!(err.to_string().contains("already registered"));
}

/// Wrong API version is rejected.
#[test]
fn test_wrong_api_version_rejected() {
    let registry = PluginRegistry::empty();
    let info = CodecPluginInfo {
        name: "bad-plugin".to_string(),
        version: "1.0.0".to_string(),
        author: "Test".to_string(),
        description: "Bad API version".to_string(),
        api_version: 999,
        license: "MIT".to_string(),
        patent_encumbered: false,
    };
    let p = Arc::new(StaticPlugin::new(info));
    let err = registry.register(p).expect_err("should fail");
    assert!(err.to_string().contains("API"));
}

/// find_decoder returns error for unknown codec.
#[test]
fn test_find_decoder_unknown_codec() {
    let registry = PluginRegistry::empty();
    registry
        .register(make_plugin("p", &[("vp9", true, true)]))
        .expect("register");
    let result = registry.find_decoder("h264");
    assert!(result.is_err());
    let err = result.err().expect("err");
    assert!(err.to_string().contains("h264"));
}

/// find_encoder returns error for unknown codec.
#[test]
fn test_find_encoder_unknown_codec() {
    let registry = PluginRegistry::empty();
    registry
        .register(make_plugin("p", &[("vp9", true, true)]))
        .expect("register");
    let result = registry.find_encoder("h265", EncoderConfig::default());
    assert!(result.is_err());
    let err = result.err().expect("err");
    assert!(err.to_string().contains("h265"));
}

/// Plugin with factory produces correct error behaviour.
#[test]
fn test_plugin_with_decoder_factory() {
    let info = CodecPluginInfo {
        name: "factory-plugin".to_string(),
        version: "1.0.0".to_string(),
        author: "Test".to_string(),
        description: "Plugin with decoder factory".to_string(),
        api_version: PLUGIN_API_VERSION,
        license: "MIT".to_string(),
        patent_encumbered: false,
    };

    let plugin = StaticPlugin::new(info)
        .add_capability(PluginCapability {
            codec_name: "test".to_string(),
            can_decode: true,
            can_encode: false,
            pixel_formats: vec![],
            properties: HashMap::new(),
        })
        .with_decoder(|codec_name| {
            Err(CodecError::UnsupportedFeature(format!(
                "Mock: decoder not available for '{codec_name}'"
            )))
        });

    let registry = PluginRegistry::empty();
    registry.register(Arc::new(plugin)).expect("register");

    assert!(registry.has_decoder("test"));
    let result = registry.find_decoder("test");
    assert!(result.is_err(), "factory must return error");
    let err = result.err().expect("error");
    assert!(err.to_string().contains("Mock"));
}
