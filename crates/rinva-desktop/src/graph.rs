//! Platform-independent graph geometry and editing commands. Coordinates are logical pixels.
use rinva_core::{Document, NodeKind, NodePosition, Project, Scene};
use std::collections::{BTreeSet, HashMap, VecDeque};

pub const NODE_WIDTH: f32 = 224.;
pub const HEADER: f32 = 72.;
pub const PORT_STEP: f32 = 28.;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn position(self) -> NodePosition {
        NodePosition {
            x: self.x.round().clamp(-100_000., 100_000.) as i32,
            y: self.y.round().clamp(-100_000., 100_000.) as i32,
        }
    }
}
impl From<NodePosition> for Point {
    fn from(p: NodePosition) -> Self {
        Self::new(p.x as f32, p.y as f32)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Port {
    Next,
    Choice(usize),
    Otherwise,
    Click,
}

pub fn ports(scene: &Scene) -> Vec<Port> {
    if scene.kind == NodeKind::End {
        vec![]
    } else if scene.kind == NodeKind::Condition {
        vec![Port::Next, Port::Otherwise]
    } else if scene.kind == NodeKind::UiButton {
        vec![Port::Next, Port::Click]
    } else if !scene.kind.interactive() || scene.choices.is_empty() {
        vec![Port::Next]
    } else {
        (0..scene.choices.len()).map(Port::Choice).collect()
    }
}
pub fn target(scene: &Scene, port: Port) -> Option<&str> {
    match port {
        Port::Next => scene.next.as_deref(),
        Port::Otherwise => scene.otherwise.as_deref(),
        Port::Click => scene.ui.as_ref().and_then(|ui| ui.target.as_deref()),
        Port::Choice(i) => scene
            .choices
            .get(i)
            .map(|c| c.target.as_str())
            .filter(|s| !s.is_empty()),
    }
}
pub fn height(scene: &Scene) -> f32 {
    HEADER + PORT_STEP * ports(scene).len().max(1) as f32 + 12.
}
pub fn output(p: Point, port: Port) -> Point {
    Point::new(
        p.x + NODE_WIDTH,
        p.y + HEADER
            + PORT_STEP
                * match port {
                    Port::Next => 0,
                    Port::Otherwise | Port::Click => 1,
                    Port::Choice(i) => i,
                } as f32
            + PORT_STEP / 2.,
    )
}
pub fn input(p: Point) -> Point {
    Point::new(p.x, p.y + 24.)
}

/// A breadth-first layout handles branches, cycles and disconnected draft scenes.
pub fn auto_layout(project: &Project) -> Vec<Point> {
    let ids: HashMap<_, _> = project
        .scenes
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let mut levels = vec![None; project.scenes.len()];
    let mut pending = VecDeque::new();
    if let Some(&start) = ids.get(project.start_scene.as_str()) {
        levels[start] = Some(0usize);
        pending.push_back(start);
    }
    while let Some(i) = pending.pop_front() {
        let depth = levels[i].unwrap();
        for port in ports(&project.scenes[i]) {
            if let Some(&j) = target(&project.scenes[i], port).and_then(|id| ids.get(id)) {
                if levels[j].is_none() {
                    levels[j] = Some(depth + 1);
                    pending.push_back(j);
                }
            }
        }
    }
    let orphan = levels.iter().flatten().copied().max().unwrap_or(0) + 1;
    let mut rows: HashMap<usize, f32> = HashMap::new();
    project
        .scenes
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let level = levels[i].unwrap_or(orphan);
            let y = rows.entry(level).or_insert(32.);
            let point = Point::new(32. + level as f32 * 300., *y);
            *y += height(s) + 48.;
            point
        })
        .collect()
}
pub fn positions(project: &Project) -> Vec<Point> {
    let mut result = auto_layout(project);
    for (i, s) in project.scenes.iter().enumerate() {
        if let Some(p) = s.node_position {
            result[i] = Point::from(p);
        }
    }
    result
}

pub fn connect(
    doc: &mut Document,
    source: usize,
    port: Port,
    destination: Option<usize>,
) -> Result<(), String> {
    let scene = doc.project.scenes.get(source).ok_or("Scene not found")?;
    if !ports(scene).contains(&port) {
        return Err("Output port not found".into());
    }
    let destination = destination
        .map(|i| {
            doc.project
                .scenes
                .get(i)
                .map(|s| s.id.clone())
                .ok_or("Scene not found")
        })
        .transpose()?;
    if target(scene, port) == destination.as_deref() {
        return Ok(());
    }
    let layout = positions(&doc.project);
    doc.change(|p| {
        freeze_positions(p, &layout);
        match port {
            Port::Next => p.scenes[source].next = destination,
            Port::Otherwise => p.scenes[source].otherwise = destination,
            Port::Click => {
                p.scenes[source]
                    .ui
                    .get_or_insert_with(Default::default)
                    .target = destination
            }
            Port::Choice(i) => p.scenes[source].choices[i].target = destination.unwrap_or_default(),
        }
    });
    Ok(())
}

pub fn freeze_positions(project: &mut Project, points: &[Point]) {
    for (scene, point) in project.scenes.iter_mut().zip(points) {
        if scene.node_position.is_none() {
            scene.node_position = Some(point.position());
        }
    }
}

#[derive(Clone, Debug)]
pub enum Drag {
    MoveMany {
        origins: Vec<(usize, Point)>,
        start: Point,
        delta: Point,
    },
    Marquee {
        start: Point,
        pointer: Point,
        original: BTreeSet<String>,
    },
    Move {
        scene: usize,
        offset: Point,
        preview: Point,
    },
    Pan {
        start: Point,
        original: Point,
    },
    Connect {
        scene: usize,
        port: Port,
        pointer: Point,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    Output(usize, Port),
    Input(usize),
    Node(usize),
    Empty,
}

pub struct Graph {
    pub zoom: f32,
    pub pan: Point,
    pub drag: Option<Drag>,
    pub selected_port: Option<(usize, Port)>,
    pub hover: Hit,
    pub middle_drag: bool,
    pub selection: BTreeSet<String>,
    pub minimap: bool,
}
impl Default for Graph {
    fn default() -> Self {
        Self {
            zoom: 1.,
            pan: Point::default(),
            drag: None,
            selected_port: None,
            hover: Hit::Empty,
            middle_drag: false,
            selection: BTreeSet::new(),
            minimap: true,
        }
    }
}
impl Graph {
    pub fn screen(&self, p: Point) -> Point {
        Point::new(p.x * self.zoom + self.pan.x, p.y * self.zoom + self.pan.y)
    }
    pub fn world(&self, p: Point) -> Point {
        Point::new(
            (p.x - self.pan.x) / self.zoom,
            (p.y - self.pan.y) / self.zoom,
        )
    }
    pub fn zoom_at(&mut self, factor: f32, anchor: Point) {
        if self.drag.is_some() {
            return;
        }
        let world = self.world(anchor);
        self.zoom = (self.zoom * factor).clamp(0.3, 2.);
        self.pan = Point::new(
            anchor.x - world.x * self.zoom,
            anchor.y - world.y * self.zoom,
        );
    }
    pub fn fit(&mut self, project: &Project, width: f32, height_px: f32) {
        self.cancel();
        let points = positions(project);
        if points.is_empty() {
            return;
        }
        let min_x = points.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let min_y = points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let max_x = points
            .iter()
            .map(|p| p.x + NODE_WIDTH)
            .fold(f32::NEG_INFINITY, f32::max);
        let max_y = points
            .iter()
            .zip(&project.scenes)
            .map(|(p, s)| p.y + height(s))
            .fold(f32::NEG_INFINITY, f32::max);
        self.zoom = ((width - 48.).max(1.) / (max_x - min_x))
            .min((height_px - 48.).max(1.) / (max_y - min_y))
            .clamp(0.3, 1.25);
        self.pan = Point::new(
            (width - (max_x - min_x) * self.zoom) / 2. - min_x * self.zoom,
            (height_px - (max_y - min_y) * self.zoom) / 2. - min_y * self.zoom,
        );
    }
    pub fn displayed_positions(&self, project: &Project) -> Vec<Point> {
        let mut points = positions(project);
        if let Some(Drag::Move { scene, preview, .. }) = self.drag {
            if let Some(p) = points.get_mut(scene) {
                *p = preview;
            }
        }
        if let Some(Drag::MoveMany { origins, delta, .. }) = &self.drag {
            for &(i, p) in origins {
                if let Some(target) = points.get_mut(i) {
                    *target = Point::new(p.x + delta.x, p.y + delta.y);
                }
            }
        }
        points
    }
    pub fn hit(&self, project: &Project, at: Point) -> Hit {
        let points = self.displayed_positions(project);
        // Ports win over card bodies; hit radii remain usable at small zoom.
        for (i, p) in points.iter().enumerate().rev() {
            for port in ports(&project.scenes[i]) {
                let q = self.screen(output(*p, port));
                if (at.x - q.x).abs() <= 10. && (at.y - q.y).abs() <= 10. {
                    return Hit::Output(i, port);
                }
            }
            let q = self.screen(input(*p));
            if (at.x - q.x).abs() <= 10. && (at.y - q.y).abs() <= 10. {
                return Hit::Input(i);
            }
        }
        let w = self.world(at);
        for (i, p) in points.iter().enumerate().rev() {
            if w.x >= p.x
                && w.x <= p.x + NODE_WIDTH
                && w.y >= p.y
                && w.y <= p.y + height(&project.scenes[i])
            {
                return Hit::Node(i);
            }
        }
        Hit::Empty
    }
    pub fn move_pointer(&mut self, at: Point) {
        let world = self.world(at);
        match &mut self.drag {
            Some(Drag::MoveMany { start, delta, .. }) => {
                *delta = Point::new((world.x - start.x).round(), (world.y - start.y).round());
            }
            Some(Drag::Marquee { pointer, .. }) => *pointer = world,
            Some(Drag::Move {
                offset, preview, ..
            }) => {
                *preview =
                    Point::from(Point::new(world.x - offset.x, world.y - offset.y).position())
            }
            Some(Drag::Pan { start, original }) => {
                self.pan = Point::new(original.x + at.x - start.x, original.y + at.y - start.y)
            }
            Some(Drag::Connect { pointer, .. }) => *pointer = at,
            None => {}
        }
    }
    pub fn cancel(&mut self) {
        if let Some(Drag::Pan { original, .. }) = self.drag.take() {
            self.pan = original;
        }
    }
    pub fn commit(&mut self, doc: &mut Document, at: Point) -> Result<(), String> {
        let hit = self.hit(&doc.project, at);
        match self.drag.take() {
            Some(Drag::MoveMany { origins, delta, .. }) => {
                if delta.x != 0. || delta.y != 0. {
                    let layout = positions(&doc.project);
                    doc.change(|p| {
                        freeze_positions(p, &layout);
                        for (i, origin) in origins {
                            if let Some(s) = p.scenes.get_mut(i) {
                                s.node_position = Some(
                                    Point::new(origin.x + delta.x, origin.y + delta.y).position(),
                                );
                            }
                        }
                    });
                }
            }
            Some(Drag::Marquee {
                start,
                pointer,
                mut original,
            }) => {
                for (s, p) in doc.project.scenes.iter().zip(positions(&doc.project)) {
                    if p.x + NODE_WIDTH >= start.x.min(pointer.x)
                        && p.x <= start.x.max(pointer.x)
                        && p.y + height(s) >= start.y.min(pointer.y)
                        && p.y <= start.y.max(pointer.y)
                    {
                        original.insert(s.id.clone());
                    }
                }
                self.selection = original;
            }
            Some(Drag::Move { scene, preview, .. }) => {
                if positions(&doc.project)
                    .get(scene)
                    .is_some_and(|p| p.position() != preview.position())
                {
                    let layout = positions(&doc.project);
                    doc.change(|p| {
                        freeze_positions(p, &layout);
                        p.scenes[scene].node_position = Some(preview.position());
                    });
                }
            }
            Some(Drag::Connect { scene, port, .. }) => {
                if let Hit::Input(target) | Hit::Node(target) = hit {
                    connect(doc, scene, port, Some(target))?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn condition_ports_and_multi_move_are_atomic() {
        let mut p = Project::demo();
        p.scenes[1].kind = NodeKind::Condition;
        let mut d = Document::new(p);
        connect(&mut d, 1, Port::Otherwise, Some(2)).unwrap();
        assert_eq!(
            d.project.scenes[1].otherwise.as_deref(),
            Some(d.project.scenes[2].id.as_str())
        );
        assert_eq!(
            ports(&d.project.scenes[1]),
            vec![Port::Next, Port::Otherwise]
        );
        let before = d.project.clone();
        let points = positions(&before);
        let mut g = Graph {
            drag: Some(Drag::MoveMany {
                origins: vec![(0, points[0]), (1, points[1])],
                start: Point::default(),
                delta: Point::default(),
            }),
            ..Graph::default()
        };
        g.move_pointer(Point::new(36., 24.));
        g.commit(&mut d, Point::default()).unwrap();
        let after = positions(&d.project);
        assert_eq!(after[0].x, points[0].x + 36.);
        assert_eq!(after[1].y, points[1].y + 24.);
        assert_eq!(after[2], points[2]);
        d.undo();
        assert_eq!(d.project, before);
        g.drag = Some(Drag::Marquee {
            start: points[0],
            pointer: Point::new(points[0].x + NODE_WIDTH, points[0].y + 40.),
            original: BTreeSet::new(),
        });
        g.commit(&mut d, Point::default()).unwrap();
        assert!(g.selection.contains(&d.project.scenes[0].id));
        assert_eq!(d.project, before);
    }
    #[test]
    fn graph_edits_drive_playback_and_undo() {
        let mut d = Document::new(Project::demo());
        let before = d.project.clone();
        connect(&mut d, 0, Port::Choice(0), Some(2)).unwrap();
        let mut player = rinva_core::Player::new(&d.project).unwrap();
        for _ in 0..3 {
            player.advance().unwrap();
        }
        player.choose(0).unwrap();
        assert!(
            matches!(player.frame(),rinva_core::Frame::Dialogue{scene,..} if scene.id==d.project.scenes[2].id)
        );
        d.undo();
        assert_eq!(d.project, before);
        d.redo();
        connect(&mut d, 0, Port::Choice(0), None).unwrap();
        assert!(d.project.check().is_err());
        assert!(connect(&mut d, 0, Port::Next, Some(1)).is_err());
    }
    #[test]
    fn cancelled_drag_is_transient_and_commit_is_one_undo() {
        let mut d = Document::new(Project::demo());
        let original = d.project.clone();
        let mut g = Graph {
            drag: Some(Drag::Move {
                scene: 0,
                offset: Point::new(10., 10.),
                preview: Point::new(32., 32.),
            }),
            ..Graph::default()
        };
        g.move_pointer(Point::new(150., 210.));
        g.cancel();
        assert_eq!(d.project, original);
        assert!(!d.can_undo());
        g.drag = Some(Drag::Move {
            scene: 0,
            offset: Point::new(10., 10.),
            preview: Point::new(32., 32.),
        });
        g.move_pointer(Point::new(150., 210.));
        g.commit(&mut d, Point::new(150., 210.)).unwrap();
        assert_eq!(
            d.project.scenes[0].node_position,
            Some(NodePosition { x: 140, y: 200 })
        );
        d.undo();
        assert_eq!(d.project, original);
        assert!(!d.can_undo());
    }
    #[test]
    fn layout_cycles_hit_testing_and_anchor_zoom() {
        let mut p = Project::demo();
        p.scenes[3].next = Some(p.scenes[0].id.clone());
        let points = auto_layout(&p);
        assert_eq!(points.len(), p.scenes.len());
        let mut g = Graph::default();
        g.fit(&p, 800., 600.);
        let pin = g.screen(output(points[0], Port::Choice(0)));
        assert_eq!(g.hit(&p, pin), Hit::Output(0, Port::Choice(0)));
        let before = g.world(pin);
        g.zoom_at(1.4, pin);
        let after = g.world(pin);
        assert!((before.x - after.x).abs() < 0.001 && (before.y - after.y).abs() < 0.001);
    }
}

#[cfg(test)]
mod interface_tests {
    use super::*;
    #[test]
    fn button_event_links_are_separate_from_construction_and_undo() {
        let project: Project =
            serde_json::from_str(include_str!("../../../examples/node-ui.rinva")).unwrap();
        let mut doc = Document::new(project.clone());
        assert_eq!(ports(&project.scenes[4]), vec![Port::Next, Port::Click]);
        connect(&mut doc, 4, Port::Click, Some(7)).unwrap();
        assert_eq!(target(&doc.project.scenes[4], Port::Next), Some("earn"));
        assert_eq!(target(&doc.project.scenes[4], Port::Click), Some("coin"));
        doc.undo();
        assert_eq!(doc.project, project);
        connect(&mut doc, 4, Port::Click, None).unwrap();
        assert!(doc.project.check().is_err());
        doc.undo();
        assert_eq!(doc.project, project);
    }
}
