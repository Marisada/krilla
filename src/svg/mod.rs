use crate::canvas::Canvas;
use tiny_skia_path::Size;
use usvg::Node;

mod clip_path;
mod filter;
mod group;
mod image;
mod mask;
mod path;
mod text;
mod util;

pub fn render_tree(tree: &usvg::Tree) -> Canvas {
    let mut canvas = Canvas::new(tree.size());
    group::render(tree.root(), &mut canvas);
    canvas
}

pub fn render_node(node: &Node) -> Canvas {
    let mut canvas = Canvas::new(
        Size::from_wh(node.bounding_box().width(), node.bounding_box().height()).unwrap(),
    );
    group::render_node(node, &mut canvas);
    canvas
}

#[cfg(test)]
mod tests {
    use crate::serialize::{PageSerialize, SerializeSettings};
    use crate::svg::render_tree;

    #[test]
    pub fn svg() {
        let data = std::fs::read("/Users/lstampfl/Programming/GitHub/svg2pdf/test.svg").unwrap();
        let tree = usvg::Tree::from_data(&data, &usvg::Options::default()).unwrap();
        let canvas = render_tree(&tree);
        let finished = canvas.serialize(SerializeSettings::default()).finish();
        let _ = std::fs::write("out/svg.pdf", &finished);
        let _ = std::fs::write("out/svg.txt", &finished);
    }
}
