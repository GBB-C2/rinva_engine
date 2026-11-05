use super::*;
use crate::graph::{Drag, Hit};

pub unsafe fn paint(app: &App, dc: HDC, rect: RECT) {
    let g = &app.graph;
    let p = app.palette;
    let scale = app.scale;
    ui::fill(dc, rect, p.window);
    let step = (24. * g.zoom * scale).max(12.) as i32;
    let start_x = ((g.pan.x * scale) as i32).rem_euclid(step);
    let start_y = ((g.pan.y * scale) as i32).rem_euclid(step);
    for x in (start_x..rect.right).step_by(step as usize) {
        for y in (start_y..rect.bottom).step_by(step as usize) {
            SetPixelV(dc, x, y, p.border);
        }
    }
    let points = g.displayed_positions(&app.document.project);
    let pixel = |world: Point| {
        let q = g.screen(world);
        POINT {
            x: (q.x * scale).round() as i32,
            y: (q.y * scale).round() as i32,
        }
    };
    let edge = |from: POINT, to: POINT, color: u32| {
        let pen = CreatePen(PS_SOLID, (2. * scale).round() as i32, color);
        let old = SelectObject(dc, pen);
        let tangent = ((to.x - from.x).abs() / 2).max((44. * scale) as i32);
        PolyBezier(
            dc,
            [
                from,
                POINT {
                    x: from.x + tangent,
                    y: from.y,
                },
                POINT {
                    x: to.x - tangent,
                    y: to.y,
                },
                to,
            ]
            .as_ptr(),
            4,
        );
        MoveToEx(
            dc,
            to.x - (6. * scale) as i32,
            to.y - (4. * scale) as i32,
            null_mut(),
        );
        LineTo(dc, to.x, to.y);
        LineTo(dc, to.x - (6. * scale) as i32, to.y + (4. * scale) as i32);
        SelectObject(dc, old);
        DeleteObject(pen);
    };
    for (i, scene) in app.document.project.scenes.iter().enumerate() {
        for port in graph::ports(scene) {
            if let Some(target) = graph::target(scene, port)
                .and_then(|id| app.document.project.scenes.iter().position(|s| s.id == id))
            {
                edge(
                    pixel(graph::output(points[i], port)),
                    pixel(graph::input(points[target])),
                    if g.selected_port == Some((i, port)) {
                        p.accent
                    } else {
                        p.muted
                    },
                );
            }
        }
    }
    if let Some(Drag::Connect {
        scene,
        port,
        pointer,
    }) = g.drag
    {
        edge(
            pixel(graph::output(points[scene], port)),
            POINT {
                x: (pointer.x * scale) as i32,
                y: (pointer.y * scale) as i32,
            },
            p.accent,
        );
    }
    let font = CreateFontW(
        -(14. * scale * g.zoom).round().max(9.) as i32,
        0,
        0,
        0,
        FW_NORMAL as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        CLEARTYPE_QUALITY as u32,
        DEFAULT_PITCH as u32,
        wide(super::fonts::family(app.language)).as_ptr(),
    );
    for (i, scene) in app.document.project.scenes.iter().enumerate() {
        let a = pixel(points[i]);
        let b = pixel(Point::new(
            points[i].x + graph::NODE_WIDTH,
            points[i].y + graph::height(scene),
        ));
        let r = RECT {
            left: a.x,
            top: a.y,
            right: b.x,
            bottom: b.y,
        };
        if r.right < 0 || r.left > rect.right || r.bottom < 0 || r.top > rect.bottom {
            continue;
        }
        let selected = i == app.selected_scene;
        let border = if selected { p.accent } else { p.border };
        ui::rounded(dc, r, p.surface, border, (4. * scale).round() as i32);
        let margin = (14. * g.zoom * scale).round() as i32;
        let header = (45. * g.zoom * scale).round() as i32;
        ui::text(
            dc,
            &scene.name,
            RECT {
                left: a.x + margin,
                top: a.y + margin / 2,
                right: b.x - margin,
                bottom: a.y + header,
            },
            p.text,
            font,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
        );
        let subtitle = if scene.id == app.document.project.start_scene {
            app.t("Начало").to_owned()
        } else {
            scene
                .lines
                .first()
                .map(|l| l.text.replace(['\r', '\n'], " "))
                .unwrap_or_else(|| app.t("Реплики").to_owned())
        };
        ui::text(
            dc,
            &subtitle,
            RECT {
                left: a.x + margin,
                top: a.y + header,
                right: b.x - margin,
                bottom: a.y + (68. * g.zoom * scale) as i32,
            },
            if scene.id == app.document.project.start_scene {
                p.accent
            } else {
                p.muted
            },
            font,
            DT_SINGLELINE | DT_END_ELLIPSIS,
        );
        let pin = pixel(graph::input(points[i]));
        draw_pin(dc, pin, scale, p, selected || g.hover == Hit::Input(i));
        for port in graph::ports(scene) {
            let pin = pixel(graph::output(points[i], port));
            let connected = graph::target(scene, port).is_some();
            let selected_port = g.selected_port == Some((i, port));
            let label = match port {
                Port::Next => {
                    if connected {
                        app.t("Переход")
                    } else {
                        app.t("Конец истории")
                    }
                }
                Port::Choice(n) => &scene.choices[n].text,
            };
            let row_h = (graph::PORT_STEP * g.zoom * scale) as i32;
            let row = RECT {
                left: a.x + 4,
                top: pin.y - row_h / 2,
                right: b.x - 4,
                bottom: pin.y + row_h / 2,
            };
            if selected_port {
                ui::rounded(dc, row, p.selected, p.selected, (4. * scale) as i32);
            }
            ui::text(
                dc,
                label,
                RECT {
                    left: a.x + margin,
                    right: b.x - margin,
                    ..row
                },
                if connected { p.text } else { p.muted },
                font,
                DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
            );
            draw_pin(
                dc,
                pin,
                scale,
                p,
                selected_port || g.hover == Hit::Output(i, port),
            );
            if !connected && matches!(port, Port::Choice(_)) {
                let pen = CreatePen(PS_SOLID, 1, p.muted);
                let old = SelectObject(dc, pen);
                MoveToEx(dc, pin.x - 2, pin.y - 2, null_mut());
                LineTo(dc, pin.x + 3, pin.y + 3);
                SelectObject(dc, old);
                DeleteObject(pen);
            }
        }
        if selected && GetFocus() == app.controls.get(CANVAS) {
            let mut focus = r;
            InflateRect(&mut focus, -3, -3);
            DrawFocusRect(dc, &focus);
        }
    }
    DeleteObject(font);
}
unsafe fn draw_pin(dc: HDC, point: POINT, scale: f32, p: Palette, active: bool) {
    let r = (5. * scale).round().max(4.) as i32;
    let brush = CreateSolidBrush(if active { p.accent } else { p.surface });
    let pen = CreatePen(
        PS_SOLID,
        (2. * scale).round() as i32,
        if active { p.accent } else { p.muted },
    );
    let ob = SelectObject(dc, brush);
    let op = SelectObject(dc, pen);
    Ellipse(
        dc,
        point.x - r,
        point.y - r,
        point.x + r + 1,
        point.y + r + 1,
    );
    SelectObject(dc, ob);
    SelectObject(dc, op);
    DeleteObject(brush);
    DeleteObject(pen);
}

pub unsafe fn input_event(app: &mut App, hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> bool {
    if !app.graph_mode || app.player.is_some() {
        return false;
    }
    let point = Point::new(
        (l as i16) as f32 / app.scale,
        ((l >> 16) as i16) as f32 / app.scale,
    );
    match msg {
        WM_LBUTTONDOWN | WM_MBUTTONDOWN => {
            app.document.finish_edit();
            SetFocus(hwnd);
            let hit = app.graph.hit(&app.document.project, point);
            if msg == WM_MBUTTONDOWN || GetKeyState(VK_SPACE as i32) < 0 || hit == Hit::Empty {
                app.graph.drag = Some(Drag::Pan {
                    start: point,
                    original: app.graph.pan,
                });
            } else {
                match hit {
                    Hit::Output(i, port) => {
                        app.selected_scene = i;
                        app.graph.selected_port = Some((i, port));
                        if let Port::Choice(n) = port {
                            app.choices_tab = true;
                            app.selected_item = n;
                        }
                        app.graph.drag = Some(Drag::Connect {
                            scene: i,
                            port,
                            pointer: point,
                        });
                    }
                    Hit::Node(i) => {
                        app.selected_scene = i;
                        app.selected_item = 0;
                        app.graph.selected_port = None;
                        app.choices_tab = false;
                        let origin = graph::positions(&app.document.project)[i];
                        let world = app.graph.world(point);
                        app.graph.drag = Some(Drag::Move {
                            scene: i,
                            offset: Point::new(world.x - origin.x, world.y - origin.y),
                            preview: origin,
                        });
                    }
                    Hit::Input(i) => {
                        app.selected_scene = i;
                        app.selected_item = 0;
                        app.graph.selected_port = None;
                    }
                    Hit::Empty => {}
                }
            }
            SetCapture(hwnd);
            app.refresh();
        }
        WM_MOUSEMOVE => {
            app.graph.move_pointer(point);
            app.graph.hover = app.graph.hit(&app.document.project, point);
            SetCursor(LoadCursorW(
                null_mut(),
                if matches!(app.graph.hover, Hit::Output(..) | Hit::Input(..)) {
                    IDC_CROSS
                } else if app.graph.drag.is_some() {
                    IDC_SIZEALL
                } else {
                    IDC_ARROW
                },
            ));
            InvalidateRect(hwnd, null(), 0);
        }
        WM_LBUTTONUP | WM_MBUTTONUP => {
            if app.graph.drag.is_some() {
                let linking = matches!(app.graph.drag, Some(Drag::Connect { .. }));
                if let Err(error) = app.graph.commit(&mut app.document, point) {
                    app.status = error;
                } else {
                    app.status = if linking {
                        "Соединение изменено · Ctrl+S — сохранить"
                    } else {
                        "Положение узлов изменено · Ctrl+S — сохранить"
                    }
                    .into();
                }
                if GetCapture() == hwnd {
                    ReleaseCapture();
                }
                app.refresh();
            }
        }
        WM_MOUSEWHEEL => {
            if app.graph.drag.is_some() {
                return true;
            }
            let delta = (w >> 16) as i16 as f32 / 120.;
            let mut at = POINT {
                x: l as i16 as i32,
                y: (l >> 16) as i16 as i32,
            };
            ScreenToClient(hwnd, &mut at);
            if GetKeyState(VK_CONTROL as i32) < 0 {
                app.graph.zoom_at(
                    1.15f32.powf(delta),
                    Point::new(at.x as f32 / app.scale, at.y as f32 / app.scale),
                );
            } else if GetKeyState(VK_SHIFT as i32) < 0 {
                app.graph.pan.x += delta * 48.;
            } else {
                app.graph.pan.y += delta * 48.;
            }
            app.controls.text(
                ZOOM_LABEL,
                &format!("{}%", (app.graph.zoom * 100.).round() as i32),
            );
            InvalidateRect(hwnd, null(), 0);
        }
        WM_CAPTURECHANGED | WM_CANCELMODE | WM_KILLFOCUS => {
            app.graph.cancel();
            InvalidateRect(hwnd, null(), 0);
        }
        WM_RBUTTONDOWN => {
            app.graph.cancel();
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
            InvalidateRect(hwnd, null(), 0);
        }
        WM_KEYDOWN => {
            match w as u16 {
                VK_ESCAPE => {
                    app.graph.cancel();
                    if GetCapture() == hwnd {
                        ReleaseCapture();
                    }
                    app.status = "Действие отменено.".into();
                    app.update_title();
                }
                VK_DELETE => {
                    if app.graph.selected_port.is_some() {
                        PostMessageW(app.hwnd, APP_COMMAND, GRAPH_UNLINK as usize, 0);
                    }
                }
                VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN => {
                    app.graph.cancel();
                    let i = app.selected_scene;
                    let mut p = graph::positions(&app.document.project)[i];
                    let step = if GetKeyState(VK_SHIFT as i32) < 0 {
                        10.
                    } else {
                        1.
                    };
                    match w as u16 {
                        VK_LEFT => p.x -= step,
                        VK_RIGHT => p.x += step,
                        VK_UP => p.y -= step,
                        _ => p.y += step,
                    }
                    app.document
                        .change(|project| project.scenes[i].node_position = Some(p.position()));
                    app.status = "Положение узлов изменено · Ctrl+S — сохранить".into();
                    app.refresh();
                }
                VK_ADD | VK_OEM_PLUS => {
                    PostMessageW(app.hwnd, APP_COMMAND, ZOOM_IN as usize, 0);
                }
                VK_SUBTRACT | VK_OEM_MINUS => {
                    PostMessageW(app.hwnd, APP_COMMAND, ZOOM_OUT as usize, 0);
                }
                0x46 => {
                    PostMessageW(app.hwnd, APP_COMMAND, GRAPH_FIT as usize, 0);
                }
                _ => return false,
            }
            InvalidateRect(hwnd, null(), 0);
        }
        _ => return false,
    }
    true
}
