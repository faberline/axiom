// HANDWRITE-BEGIN gap="missing-generator:unit-test:surface-reexport" tracker="standardize-gap-projects-jet-wasm-tests-surface-snapshot-rs" reason="Jet-wasm compatibility test for the surface re-export and snapshot API."
use jet_wasm::{Callback, Element, Props};

#[test]
fn jet_wasm_reexports_surface_snapshot_core() {
    let element = Element::intrinsic(
        "button",
        Props {
            id: Some("confirm".to_string()),
            on_click: Some(Callback::new(|_| {})),
            ..Default::default()
        },
        vec![Element::text("Confirm")],
    );

    let snapshot = element.surface_snapshot();
    let button = snapshot.find_by_semantic_id("confirm").unwrap();
    assert_eq!(button.role.as_deref(), Some("button"));
    assert_eq!(button.name.as_deref(), Some("Confirm"));
    assert!(button.props.has_on_click);
}
// HANDWRITE-END
