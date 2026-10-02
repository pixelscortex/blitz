//! Layout rounding must use the device-pixel grid, including at fractional
//! display scales and document zoom (DioxusLabs/blitz#837).

use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use blitz_traits::shell::{ColorScheme, Viewport};
use taffy::Layout;

fn document(html: &str, scale: f32, zoom: f32) -> HtmlDocument {
    let mut viewport = Viewport::new(1080, 2400, scale, ColorScheme::Light);
    viewport.zoom = zoom;
    let mut doc = HtmlDocument::from_html(
        html,
        DocumentConfig {
            viewport: Some(viewport),
            ..Default::default()
        },
    );
    doc.resolve(0.0);
    doc
}

fn layout(doc: &HtmlDocument, id: &str) -> Layout {
    let id = doc.get_element_by_id(id).unwrap();
    *doc.get_node(id).unwrap().final_layout()
}

fn close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1e-3,
        "expected {expected}, got {actual}"
    );
}

const BORDER_HTML: &str = r#"<html><head><style>
    body { margin: 0; height: 100vh; display: flex;
        align-items: center; justify-content: center; }
    #box { box-sizing: border-box; width: 113px; height: 44px;
        border: 1px solid #999; }
    </style></head><body><div id="box"></div></body></html>"#;

#[test]
fn thin_borders_survive_fractional_display_scales_and_zoom() {
    for (scale, zoom) in [
        (1.0, 1.0),
        (1.25, 1.0),
        (1.5, 1.0),
        (2.0, 1.0),
        (2.75, 1.0),
        (1.0, 1.25),
        (2.0, 1.375),
    ] {
        let doc = document(BORDER_HTML, scale, zoom);
        let id = doc.get_element_by_id("box").unwrap();
        let node = doc.get_node(id).unwrap();
        let before = node.unrounded_layout().border;
        let after = node.final_layout().border;
        let scale = scale * zoom;
        for (side, original, rounded) in [
            ("left", before.left, after.left),
            ("right", before.right, after.right),
            ("top", before.top, after.top),
            ("bottom", before.bottom, after.bottom),
        ] {
            assert!(rounded > 0.0, "{side} vanished at scale {scale}: {after:?}");
            close(rounded * scale, (original * scale).round());
        }
    }
}

#[test]
fn adjacent_nested_boxes_share_a_device_pixel_edge() {
    let html = r#"<html><body style="margin:0">
        <div id="outer" style="position:relative; left:0.2px">
          <div id="row" style="display:flex; position:relative; left:0.3px">
            <div id="a" style="width:10.2px; height:10px; flex:none"></div>
            <div id="b" style="width:10.2px; height:10px; flex:none"></div>
          </div>
        </div></body></html>"#;
    for scale in [1.0, 1.25, 1.5, 2.0, 2.75] {
        let doc = document(html, scale, 1.0);
        let outer = layout(&doc, "outer");
        let row = layout(&doc, "row");
        let a = layout(&doc, "a");
        let b = layout(&doc, "b");
        let origin = outer.location.x + row.location.x;
        close(origin * scale, (0.5 * scale).round());
        close(a.location.x + a.size.width, b.location.x);
        close((origin + b.location.x) * scale, (10.7 * scale).round());
    }
}

#[test]
fn scale_changes_reround_cached_layout() {
    let mut doc = document(BORDER_HTML, 1.0, 1.0);
    for scale in [2.75, 1.25, 1.0] {
        doc.viewport_mut().hidpi_scale = scale;
        doc.resolve(0.0);
        let id = doc.get_element_by_id("box").unwrap();
        let node = doc.get_node(id).unwrap();
        close(
            node.final_layout().border.left * scale,
            (node.unrounded_layout().border.left * scale).round(),
        );
        close(
            node.final_layout().border.right * scale,
            (node.unrounded_layout().border.right * scale).round(),
        );
    }
}

#[test]
fn hoisted_absolute_child_rounds_relative_to_its_containing_block() {
    let html = r#"<html><body style="margin:0">
        <div id="outer" style="position:relative; left:0.2px">
          <div style="margin-left:20px">
            <div id="absolute" style="position:absolute; left:10.3px;
              top:0; width:10.2px; height:10px"></div>
          </div>
        </div></body></html>"#;
    for scale in [1.0, 1.25, 1.5, 2.0, 2.75] {
        let doc = document(html, scale, 1.0);
        let outer = layout(&doc, "outer");
        let absolute = layout(&doc, "absolute");
        let left = outer.location.x + absolute.location.x;
        close(left * scale, (10.5 * scale).round());
        close((left + absolute.size.width) * scale, (20.7 * scale).round());
    }
}

#[test]
fn zero_width_border_sides_stay_zero() {
    let html = r#"<html><body style="margin:0">
        <div id="box" style="width:100px; height:40px; border:1px solid;
          border-top-width:0; border-left-width:2px; padding:3.2px"></div>
        </body></html>"#;
    for scale in [1.0, 1.25, 1.5, 2.0, 2.75] {
        let doc = document(html, scale, 1.0);
        let id = doc.get_element_by_id("box").unwrap();
        let node = doc.get_node(id).unwrap();
        let before = node.unrounded_layout().border;
        let after = node.final_layout().border;
        assert_eq!(after.top, 0.0);
        close(after.left * scale, (before.left * scale).round());
        close(after.right * scale, (before.right * scale).round());
        close(after.bottom * scale, (before.bottom * scale).round());
    }
}
