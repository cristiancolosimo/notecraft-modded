//! The Insert tab, laid out to a drawn design with the same measures and the same way of building a
//! group as the View tab ([`crate::viewtab`]): Insert, Tables, File, Images, Links, Date and time,
//! Symbols, More.

use crate::ribbon::{Act, View};
use crate::svgicon::Prim;
use crate::theme::Tokens;
use crate::viewtab::{CONTENT_TOP, Face, Ink, Spec, group};
use egui::{Rect, Sense, Ui, pos2};

const SPACE: [Prim; 1] = [Prim::Path("M4 4h16M4 20h16M12 8v8M9 11l3-3 3 3M9 13l3 3 3-3")];
const TABLE: [Prim; 2] = [Prim::Rect(3.0, 4.0, 18.0, 16.0, 2.0), Prim::Path("M3 10h18M3 15h18M9 4v16M15 4v16")];
const PRINTOUT: [Prim; 2] = [Prim::Path("M14 3H6a2 2 0 00-2 2v14a2 2 0 002 2h12a2 2 0 002-2V9z"), Prim::Path("M14 3v6h6M8 13h8M8 17h5")];
const ATTACHMENT: [Prim; 1] = [Prim::Path("M20 11.5l-8.3 8.3a5 5 0 01-7-7l8.6-8.6a3.3 3.3 0 014.7 4.7l-8.6 8.6a1.7 1.7 0 01-2.4-2.4l7.9-7.9")];
const PICTURE: [Prim; 3] = [Prim::Rect(3.0, 4.0, 18.0, 16.0, 2.0), Prim::Circle(9.0, 10.0, 1.5), Prim::Path("M21 16l-5-5-9 9")];
const LINK: [Prim; 2] = [Prim::Path("M10 14a4 4 0 005.7 0l3-3a4 4 0 00-5.7-5.7l-1 1"), Prim::Path("M14 10a4 4 0 00-5.7 0l-3 3a4 4 0 005.7 5.7l1-1")];
const DATE: [Prim; 2] = [Prim::Rect(3.0, 5.0, 18.0, 16.0, 2.0), Prim::Path("M3 10h18M8 3v4M16 3v4")];
const TIME: [Prim; 2] = [Prim::Circle(12.0, 12.0, 9.0), Prim::Path("M12 7v5l3 2")];
const DATE_TIME: [Prim; 3] = [
    Prim::Path("M21 10V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2h5M3 10h18M8 3v4M16 3v4"),
    Prim::Circle(17.0, 17.0, 4.0),
    Prim::Path("M17 15.5V17l1 1"),
];
const EQUATION: [Prim; 1] = [Prim::Path("M18 5H7l6 7-6 7h11")];
const SYMBOL: [Prim; 1] = [Prim::Path("M4 20h5v-3a7 7 0 116 0v3h5")];
const DIAGRAM: [Prim; 3] = [Prim::Rect(3.0, 3.0, 7.0, 6.0, 1.5), Prim::Rect(14.0, 15.0, 7.0, 6.0, 1.5), Prim::Path("M6.5 9v5a2 2 0 002 2H14")];

fn plain<'a>(name: &'a str, lines: &'a [&'a str], icon: &'a [Prim]) -> Face<'a> {
    Face { name, lines, icon, on: false, caret: false }
}

fn menu<'a>(name: &'a str, lines: &'a [&'a str], icon: &'a [Prim]) -> Face<'a> {
    Face { name, lines, icon, on: false, caret: true }
}

/// What a press on button `i` of a group of plain buttons asks for.
fn press(acts: &mut Vec<Act>, resp: Option<egui::Response>, act: Act) {
    if resp.is_some_and(|r| r.clicked()) {
        acts.push(act);
    }
}

/// The Insert tab.
pub fn insert_tab(ui: &mut Ui, view: &View<'_>, t: &Tokens, acts: &mut Vec<Act>) {
    let origin = ui.cursor().min;
    let panel_top = origin.y - CONTENT_TOP;
    let ink = Ink::new(t);
    // The first group's content begins 6 from the content's edge, which is 8 from the window's.
    let mut x = origin.x + 6.0;

    x = group(ui, &ink, panel_top, x, "Insert", true, &[Spec::Button(plain("Insert Space", &["Space"], &SPACE), "space")], |_, _, _, r| {
        press(acts, r, Act::Insert("space"));
    });
    x = group(ui, &ink, panel_top, x, "Tables", true, &[Spec::Button(menu("Table", &["Table"], &TABLE), "table")], |_, _, _, r| {
        if let Some(r) = r {
            crate::ribbon::table_popup(&r, view, t, acts);
        }
    });
    x = group(
        ui,
        &ink,
        panel_top,
        x,
        "Files",
        true,
        &[
            Spec::Button(plain("File Printout", &["Printout", "of a file"], &PRINTOUT), "printout"),
            Spec::Button(plain("File Attachment", &["Attachment"], &ATTACHMENT), "attachment"),
        ],
        |_, i, _, r| press(acts, r, if i == 0 { Act::AddPrintout } else { Act::AddFile }),
    );
    x = group(ui, &ink, panel_top, x, "Images", true, &[Spec::Button(plain("Pictures", &["Picture"], &PICTURE), "picture")], |_, _, _, r| {
        press(acts, r, Act::AddPicture);
    });
    x = group(ui, &ink, panel_top, x, "Links", true, &[Spec::Button(plain("Link", &["Link"], &LINK), "link")], |_, _, _, r| {
        press(acts, r, Act::Insert("link"));
    });
    x = group(
        ui,
        &ink,
        panel_top,
        x,
        "Time Stamp",
        true,
        &[
            Spec::Button(plain("Date", &["Date"], &DATE), "date"),
            Spec::Button(plain("Time", &["Time"], &TIME), "time"),
            Spec::Button(plain("Date & Time", &["Date & time"], &DATE_TIME), "datetime"),
        ],
        |_, i, _, r| press(acts, r, Act::Insert(["date", "time", "datetime"][i])),
    );
    x = group(
        ui,
        &ink,
        panel_top,
        x,
        "Symbols",
        true,
        &[Spec::Button(menu("Equation", &["Equation"], &EQUATION), "equation"), Spec::Button(menu("Symbol", &["Symbol"], &SYMBOL), "symbol")],
        |_, i, _, r| match (i, r) {
            (0, Some(r)) => crate::ribbon::equation_popup(&r, t, acts),
            (1, r) => press(acts, r, Act::Symbols),
            _ => {}
        },
    );
    x = group(ui, &ink, panel_top, x, "More", false, &[Spec::Button(plain("Diagram", &["Diagram"], &DIAGRAM), "diagram")], |_, _, _, r| {
        press(acts, r, Act::Insert("diagram"));
    });

    ui.allocate_rect(Rect::from_min_max(origin, pos2(x, panel_top + CONTENT_TOP + 100.0)), Sense::hover());
}
