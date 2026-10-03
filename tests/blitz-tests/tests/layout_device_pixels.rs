//! Layout rounding must use the device-pixel grid, including at fractional
//! display scales and document zoom (DioxusLabs/blitz#837).

use blitz_dom::DocumentConfig;
use blitz_html::{HtmlDocument, HtmlProvider};
use blitz_traits::shell::{ColorScheme, Viewport};
use std::sync::Arc;
use taffy::Layout;

fn document(html: &str, scale: f32, zoom: f32) -> HtmlDocument {
    let mut viewport = Viewport::new(1080, 2400, scale, ColorScheme::Light);
    viewport.zoom = zoom;
    let mut doc = HtmlDocument::from_html(
        html,
        DocumentConfig {
            viewport: Some(viewport),
            html_parser_provider: Some(Arc::new(HtmlProvider) as _),
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
    // Fixed device-pixel expectations for a 1 CSS px border. Stylo snaps down
    // to whole device pixels, with a minimum of one for nonzero borders.
    for (scale, zoom, expected) in [
        (1.0, 1.0, 1.0),
        (1.25, 1.0, 1.0),
        (1.5, 1.0, 1.0),
        (2.0, 1.0, 2.0),
        (2.75, 1.0, 2.0),
        (1.0, 1.25, 1.0),
        (2.0, 1.375, 2.0),
    ] {
        let doc = document(BORDER_HTML, scale, zoom);
        let id = doc.get_element_by_id("box").unwrap();
        let node = doc.get_node(id).unwrap();
        let after = node.final_layout().border;
        let scale = scale * zoom;
        for (side, rounded) in [
            ("left", after.left),
            ("right", after.right),
            ("top", after.top),
            ("bottom", after.bottom),
        ] {
            assert!(rounded > 0.0, "{side} vanished at scale {scale}: {after:?}");
            close(rounded * scale, expected);
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
    let html = BORDER_HTML.replace("height: 100vh", "height: 100px");
    let mut doc = document(&html, 1.0, 1.0);
    doc.set_incremental_layout(true);
    for (scale, zoom, expected) in [
        (2.75, 1.0, 2.0),
        (1.25, 1.0, 1.0),
        (1.0, 1.0, 1.0),
        (2.0, 1.375, 2.0),
        (1.0, 1.25, 1.0),
    ] {
        {
            let mut viewport = doc.viewport_mut();
            viewport.hidpi_scale = scale;
            viewport.zoom = zoom;
        }
        doc.resolve(0.0);
        let id = doc.get_element_by_id("box").unwrap();
        let node = doc.get_node(id).unwrap();
        close(node.final_layout().border.left * scale * zoom, expected);
        close(node.final_layout().border.right * scale * zoom, expected);
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
    for (scale, left, other) in [
        (1.0, 2.0, 1.0),
        (1.25, 2.0, 1.0),
        (1.5, 3.0, 1.0),
        (2.0, 4.0, 2.0),
        (2.75, 5.0, 2.0),
    ] {
        let doc = document(html, scale, 1.0);
        let id = doc.get_element_by_id("box").unwrap();
        let node = doc.get_node(id).unwrap();
        let after = node.final_layout().border;
        assert_eq!(after.top, 0.0);
        close(after.left * scale, left);
        close(after.right * scale, other);
        close(after.bottom * scale, other);
    }
}

#[test]
fn painted_borders_keep_their_thickness_with_and_without_children() {
    for display in ["block", "flex", "grid", "table"] {
        for with_child in [false, true] {
            check_painted_borders(display, with_child);
        }
    }
}

fn check_painted_borders(display: &str, with_child: bool) {
    use anyrender::render_to_buffer;
    use anyrender_vello_cpu::VelloCpuImageRenderer;
    use blitz_paint::paint_scene;

    let mut html = BORDER_HTML
        .replace("113px", "44px")
        .replace("height: 44px", "height: 24px")
        .replace("#999", "red")
        .replace(
            "box-sizing: border-box",
            &format!("display: {display}; box-sizing: border-box"),
        );
    if with_child {
        html = html.replace(
            "<div id=\"box\"></div>",
            "<div id=\"box\"><div style=\"width:100%;height:100%;background:blue\"></div></div>",
        );
    }
    if display == "table" {
        html = html.replace("display: table;", "display: table; border-spacing: 0;");
        html = html
            .replace(
                "<div id=\"box\">",
                "<table id=\"box\"><tbody><tr><td style=\"padding:0\">",
            )
            .replace("</div></body>", "</td></tr></tbody></table></body>");
    }
    let mut doc = document(&html, 2.75, 1.0);
    doc.set_viewport(Viewport::new(264, 176, 2.75, ColorScheme::Light));
    doc.resolve(0.0);
    let buffer = render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| paint_scene(scene, &mut doc, 2.75, 264, 176, 0, 0),
        264,
        176,
    );
    if with_child {
        let center = (88 * 264 + 132) * 4;
        assert!(
            buffer[center + 2] > 200 && buffer[center] < 50,
            "{display}: child background must actually paint"
        );
    }
    let red = |x: usize, y: usize| {
        let i = (y * 264 + x) * 4;
        buffer[i] > 200 && buffer[i + 1] < 50 && buffer[i + 2] < 50
    };
    // The centered 44x24 CSS px box covers [72,193) x [55,121) device
    // pixels. Each side must paint two pixels, independently of layout data.
    for (edge, count) in [
        ("left", (0..132).filter(|&x| red(x, 88)).count()),
        ("right", (132..264).filter(|&x| red(x, 88)).count()),
        ("top", (0..88).filter(|&y| red(132, y)).count()),
        ("bottom", (88..176).filter(|&y| red(132, y)).count()),
    ] {
        assert_eq!(
            count, 2,
            "{display}, child={with_child}: {edge} border painted {count} pixels"
        );
    }
}
