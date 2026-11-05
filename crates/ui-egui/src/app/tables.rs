//! Tables, and the spreadsheet they can be handed to.

use super::*;

impl NoteCraftApp {
    pub(super) fn caret_in_table(&self) -> bool {
        self.editor
            .as_ref()
            .and_then(|e| e.caret.as_ref().map(|c| (e, c)))
            .is_some_and(|(e, c)| c.cell.is_some() && matches!(e.blocks.get(c.block), Some(notecraft_formats::markdown::Block::Table { .. })))
    }

    /// Puts the spreadsheet back into the table it came from.
    ///
    /// The file is the one Convert wrote for this table, beside the page. Nothing watches
    /// it: a page would be a strange thing to write in if its tables changed under the
    /// writer's hands, so the way back is a button, as the way out is.
    pub(super) fn read_table_from_gridcraft(&mut self) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some(at) = ed.caret.as_ref().map(|c| c.block) else {
            self.say("Click in the table first");
            return;
        };
        if !matches!(ed.blocks.get(at), Some(notecraft_formats::markdown::Block::Table { .. })) {
            self.say("Click in a table first");
            return;
        }
        let file = ed.dir.join(format!("table-{}.csv", at + 1));
        let Ok(text) = std::fs::read_to_string(&file) else {
            let name = file.file_name().unwrap_or_default().to_string_lossy().into_owned();
            self.complain(&format!("No {name} beside this page: hand the table over first"));
            return;
        };
        let Some(table) = notecraft_formats::csv::to_table(&text) else {
            self.complain("That file has nothing in it to put back");
            return;
        };
        let (rows, columns) = pageedit::table_size(&table).unwrap_or((0, 0));
        if let Some(block) = ed.blocks.get_mut(at) {
            *block = table;
        }
        ed.caret = None;
        ed.body = pageedit::to_body(ed.format, &ed.blocks);
        Self::mark_dirty_from_blocks(ed);
        self.say(format!("Read back {rows} rows and {columns} columns"));
    }

    /// Hands the table the caret is in to GridCraft, which is where a spreadsheet
    /// belongs: OneNote's Table tab ends with Convert to Excel Spreadsheet, and this is
    /// the same door with the open spreadsheet behind it.
    pub(super) fn hand_table_to_gridcraft(&mut self) {
        let Some(exe) = self.gridcraft.clone() else {
            self.say("GridCraft is not installed");
            return;
        };
        let Some(ed) = self.editor.as_ref() else { return };
        let Some(at) = ed.caret.as_ref().map(|c| c.block) else { return };
        let Some(block) = ed.blocks.get(at) else { return };
        let name = format!("table-{}", at + 1);
        match crate::gridcraft::hand_over(&exe, &ed.dir, &name, block) {
            Ok(file) => {
                let said = format!("{} is open in GridCraft", file.file_name().unwrap_or_default().to_string_lossy());
                self.say(&said);
            }
            Err(e) => self.complain(&format!("Cannot open the table in GridCraft: {e}")),
        }
    }

    /// A table of the picked size, where the caret is.
    pub(super) fn insert_table(&mut self, rows: usize, columns: usize) {
        let Some(ed) = self.editor.as_mut() else { return };
        let at = ed.caret.as_ref().map(|c| c.block + 1).unwrap_or(ed.blocks.len());
        pageedit::insert_table(&mut ed.blocks, at, rows, columns);
        ed.rebuild_body();
        ed.caret = Some(pageedit::Caret {
            block: at.min(ed.blocks.len() - 1),
            text: String::new(),
            focus: true,
            selection: (0, 0),
            place: None,
            cell: Some((0, 0)),
            span: None,
        });
        Self::mark_dirty_from_blocks(ed);
    }

    /// An empty spreadsheet beside the page, opened in GridCraft and linked from the page
    /// so it can be found again.
    pub(super) fn new_gridcraft(&mut self) {
        let Some(exe) = self.gridcraft.clone() else {
            self.say("GridCraft is not installed");
            return;
        };
        let Some(ed) = self.editor.as_ref() else { return };
        let dir = ed.dir.clone();
        let mut name = "spreadsheet.csv".to_string();
        for n in 2..100 {
            if !dir.join(&name).exists() {
                break;
            }
            name = format!("spreadsheet-{n}.csv");
        }
        let file = dir.join(&name);
        if let Err(e) = std::fs::write(&file, "") {
            self.complain(&format!("Cannot write {name}: {e}"));
            return;
        }
        match std::process::Command::new(exe).arg(&file).spawn() {
            Ok(_) => {
                self.insert_in_page(&format!("[{name}]({name})"));
                let said = format!("{name} is open in GridCraft");
                self.say(&said);
            }
            Err(e) => self.complain(&format!("Cannot open GridCraft: {e}")),
        }
    }

    /// Picks a block of cells from the Table tab's Select buttons, as a drag across them
    /// would.
    pub(super) fn select_in_table(&mut self, what: pageedit::TableSpan) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some((at, cell)) = ed.caret.as_ref().and_then(|c| c.cell.map(|cell| (c.block, cell))) else {
            return;
        };
        let Some(block) = ed.blocks.get(at) else { return };
        let Some((from, to)) = pageedit::span_of(block, cell, what) else { return };
        if let Some(caret) = ed.caret.as_mut() {
            caret.cell = Some(from);
            caret.span = Some(to);
            caret.text = pageedit::cell_text(block, from).unwrap_or_default();
            caret.focus = false;
        }
    }

    /// Changes the shape of the table the caret is in, from the Table tab.
    ///
    /// A block of cells picked across several rows or columns takes them all: Delete Rows
    /// with three rows picked takes the three, the way it does in OneNote.
    pub(super) fn edit_table(&mut self, what: pageedit::TableEdit) {
        let Some(ed) = self.editor.as_mut() else { return };
        let Some((at, cell)) = ed.caret.as_ref().and_then(|c| c.cell.map(|cell| (c.block, cell))) else {
            return;
        };
        // Deleting works from the top-left of what is picked, over and over: each one
        // after the first falls into the place the one before it left. A picked block
        // that takes in the heading row starts under it, so picking the whole table and
        // asking for its rows empties the body and keeps the heading.
        let picked = ed.caret.as_ref().and_then(pageedit::cell_range);
        let (cell, times) = match (what, picked) {
            (pageedit::TableEdit::DeleteRow, Some((a, b))) => {
                let start = a.0.max(1);
                ((start, a.1), (b.0 + 1).saturating_sub(start))
            }
            (pageedit::TableEdit::DeleteColumn, Some((a, b))) => (a, b.1 - a.1 + 1),
            _ => (cell, 1),
        };
        let Some(block) = ed.blocks.get_mut(at) else { return };
        let mut last = None;
        for _ in 0..times {
            match pageedit::edit_table(block, cell, what) {
                Some(to) => last = Some(to),
                None => break,
            }
        }
        let Some(to) = last else {
            // The rules said no: a table keeps its heading row and at least one column.
            self.say("A table keeps its heading row and at least one column");
            return;
        };
        let text = pageedit::cell_text(block, to).unwrap_or_default();
        ed.rebuild_body();
        ed.caret = Some(pageedit::Caret { block: at, text, focus: true, selection: (0, 0), place: None, cell: Some(to), span: None });
        Self::mark_dirty_from_blocks(ed);
    }
}
