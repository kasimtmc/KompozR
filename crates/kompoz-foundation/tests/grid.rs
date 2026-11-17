//! Grid, as Compose's foundation-layout Grid: its tracks sized (fixed, percentage, flexible, min-max, content,
//! auto, spanning items), its items placed (by index, range, named area, the flow), implicit tracks, gaps, right to
//! left, intrinsics, its config (the C++ Kompoz's Grid tests on Compose's API, and what they don't reach).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_runtime::*;
use kompoz_test::*;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::*;
use std::cell::Cell;

// ---- Helpers ---------------------------------------------------------------------------------------------------

/// A node's outer bounds in the root: x, y, width, height.
fn bounds(rule: &KompozTestRule, tag: &str) -> [f32; 4] {
    let b = rule.on_node_with_tag(tag).get_unclipped_bounds_in_root();
    [b.left, b.top, b.width(), b.height()]
}

/// Where a node is in a grid's node: x, y.
fn at(rule: &KompozTestRule, grid: &str, tag: &str) -> (f32, f32) {
    let (g, r) = (bounds(rule, grid), bounds(rule, tag));
    (r[0] - g[0], r[1] - g[1])
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

/// C++'s GridLeaf: as wide as `max` (its min intrinsic width `min`), and `area` / its width tall, as a text is.
struct GridLeafPolicy {
    min: i32,
    max: i32,
    area: i32,
}

impl GridLeafPolicy {
    fn height_at(&self, width: i32) -> i32 {
        let w = self.max.min(width);
        if w <= 0 { 0 } else { self.area / w }
    }
}

impl MeasurePolicy for GridLeafPolicy {
    fn measure(&self, scope: &MeasureScope, _: &[&dyn Measurable], c: Constraints) -> MeasureResult {
        let w = self.max.clamp(c.min_width, c.max_width);
        let h = self.height_at(w).clamp(c.min_height, c.max_height);
        scope.layout(w, h, |_| {})
    }
    fn min_intrinsic_width(&self, _: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], _: i32) -> i32 {
        self.min
    }
    fn max_intrinsic_width(&self, _: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], _: i32) -> i32 {
        self.max
    }
    fn min_intrinsic_height(&self, _: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], width: i32) -> i32 {
        self.height_at(width)
    }
    fn max_intrinsic_height(&self, _: &IntrinsicMeasureScope, _: &[&dyn IntrinsicMeasurable], width: i32) -> i32 {
        self.height_at(width)
    }
}

#[kompozable(inline)]
fn GridLeaf(min: i32, max: i32, area: i32, modifier: Modifier) {
    Layout(modifier = modifier, measure_policy = GridLeafPolicy { min, max, area });
}

thread_local! {
    /// How many times a counting config ran.
    static RUNS: Cell<u32> = const { Cell::new(0) };
}

fn runs() -> u32 {
    RUNS.with(|r| r.get())
}

// ---- Ported from C++ -------------------------------------------------------------------------------------------

fn grid_sizes_tracks_as_compose() {
    // C++'s `grid_sizes_tracks_as_compose` (kompoz_tests.cpp 15990), its `GridConfig`s config blocks.
    use GridTrackSize as T;
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(|| {
            Column(
                content = || {
                    // Fixed, then two flexible columns sharing what's left after their min content.
                    Grid(
                        config = |c: &mut GridConfigurationScope| {
                            c.columns([T::Fixed(50.dp()), T::Flex(1.fr()), T::Flex(2.fr())]);
                            c.rows([T::Fixed(40.dp()), T::Auto]);
                            c.gap_rc(5.dp(), 10.dp());
                        },
                        modifier = Modifier.width(300.dp()).test_tag("g"),
                        content = || {
                            Box(Modifier.size_wh(20.dp(), 20.dp()).test_tag("a"));
                            Box(Modifier.size_wh(30.dp(), 30.dp()).test_tag("b"));
                            Box(Modifier.size_wh(30.dp(), 30.dp()).test_tag("c"));
                            Box(Modifier.size_wh(10.dp(), 60.dp()).test_tag("d"));
                        },
                    );
                    // A percentage of the room after the gaps; MinMax from its minimum.
                    Grid(
                        config = |c: &mut GridConfigurationScope| {
                            c.columns([T::Percentage(0.5), T::MinMax(40.dp(), 1.fr())]);
                            c.column_gap(10.dp());
                        },
                        modifier = Modifier.width(210.dp()),
                        content = || {
                            Box(Modifier.fill_max_width().height(10.dp()).test_tag("yarım"));
                            Box(Modifier.fill_max_width().height(10.dp()).test_tag("kalan"));
                        },
                    );
                    // Auto columns grow from min towards max content by how much they can.
                    Grid(
                        config = |c: &mut GridConfigurationScope| c.columns([T::Auto, T::Auto]),
                        modifier = Modifier.width(200.dp()),
                        content = || {
                            GridLeaf(20, 100, 1000, Modifier.test_tag("oto1"));
                            GridLeaf(40, 200, 1000, Modifier.test_tag("oto2"));
                        },
                    );
                    // Rows are as tall as their items at their column's width.
                    Grid(
                        config = |c: &mut GridConfigurationScope| {
                            c.columns([T::Fixed(40.dp()), T::MinContent, T::MaxContent])
                        },
                        modifier = Modifier,
                        content = || {
                            GridLeaf(10, 400, 2000, Modifier.test_tag("metin"));
                            GridLeaf(10, 400, 2000, Modifier.test_tag("dar"));
                            GridLeaf(10, 400, 2000, Modifier.test_tag("geniş"));
                        },
                    );
                    // One across a fixed and a content column grows only the content one.
                    Grid(
                        config = |c: &mut GridConfigurationScope| c.columns([T::Fixed(30.dp()), T::MinContent]),
                        modifier = Modifier.test_tag("yayılan"),
                        content = || {
                            Box(Modifier.size_wh(100.dp(), 10.dp()).grid_item(row = 1, column = 1, column_span = 2));
                            Box(Modifier.size_wh(5.dp(), 10.dp()).grid_item(row = 2, column = 2).test_tag("büyüyen"));
                        },
                    );
                    // Unbounded: a percentage is the content's size; flexible tracks keep their minimum.
                    let scroll = remember_scroll_state();
                    Row(
                        Modifier.horizontal_scroll(&scroll),
                        content = || {
                            Grid(
                                config =
                                    |c: &mut GridConfigurationScope| c.columns([T::Percentage(0.5), T::Flex(1.fr())]),
                                modifier = Modifier.test_tag("sınırsız"),
                                content = || {
                                    Box(Modifier.size_wh(70.dp(), 10.dp()));
                                    GridLeaf(15, 90, 150, Modifier.test_tag("esnek"));
                                },
                            );
                        },
                    );
                },
            );
        }),
        IntSize::new(400, 2000),
    );
    let g = bounds(&rule, "g");
    assert!(near(g[2], 300.0) && near(g[3], 105.0));
    assert_eq!(at(&rule, "g", "a"), (0.0, 0.0));
    assert_eq!(at(&rule, "g", "b"), (60.0, 0.0));
    // C++ expected 60 + 30 + 170/3 + 10, its tracks floats; Compose's are whole px, the flexible shares rounded as
    // they accumulate: round(170/3) = 57 to the first, 113 to the second.
    assert_eq!(at(&rule, "g", "c"), (60.0 + 30.0 + 57.0 + 10.0, 0.0));
    assert_eq!(at(&rule, "g", "d"), (0.0, 45.0));
    let (half, rest) = (bounds(&rule, "yarım"), bounds(&rule, "kalan"));
    assert!(near(half[2], 100.0) && near(rest[2], 100.0) && near(rest[0], half[0] + 110.0));
    // C++ expected 20 + 80 * 140/240 and 40 + 160 * 140/240 (66.67 and 133.33); Compose rounds each Auto column's
    // share of the room: 47 and 93.
    assert!(near(bounds(&rule, "oto1")[2], 67.0) && near(bounds(&rule, "oto2")[2], 133.0));
    let size = |tag| {
        let b = bounds(&rule, tag);
        (b[2], b[3])
    };
    assert_eq!(size("metin"), (40.0, 50.0));
    assert_eq!(size("dar"), (10.0, 200.0));
    assert_eq!(size("geniş"), (400.0, 5.0));
    assert_eq!(at(&rule, "yayılan", "büyüyen").0, 30.0);
    assert!(near(bounds(&rule, "büyüyen")[2], 5.0) && near(bounds(&rule, "yayılan")[2], 100.0));
    assert!(near(bounds(&rule, "sınırsız")[2], 70.0 + 15.0) && near(bounds(&rule, "esnek")[2], 15.0));
}

fn grid_places_items_as_compose() {
    // C++'s `grid_places_items_as_compose` (kompoz_tests.cpp 16047): its indices were Compose's (1-based, 0 for
    // the flow), its `gridItem(row, column, rowSpan, columnSpan)` Compose's, its areas a config's `area`.
    let three = [GridTrackSize::Fixed(10.dp()); 3];
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        Column(
            content = || {
                Grid(
                    config = move |c: &mut GridConfigurationScope| c.columns(three),
                    modifier = Modifier.test_tag("g"),
                    content = || {
                        // Where asked.
                        Box(Modifier.size(10.dp()).grid_item(row = 2, column = 3).test_tag("a"));
                        // The first free cells.
                        Box(Modifier.size(10.dp()).grid_item(column_span = 2).test_tag("b"));
                        // Doesn't fit after b: the next row.
                        Box(Modifier.size(10.dp()).grid_item(column_span = 2).test_tag("c"));
                        // a is there: the next row.
                        Box(Modifier.size(10.dp()).test_tag("d"));
                        // No rows configured: placed by the flow.
                        Box(Modifier.size(10.dp()).grid_item(row = -1).test_tag("e"));
                        // The first free row of the first column.
                        Box(Modifier.size(10.dp()).grid_item(column = 1).test_tag("f"));
                        // An unknown area: placed by the flow.
                        Box(Modifier.size(10.dp()).grid_item_area("yok").test_tag("g1"));
                    },
                );
                Grid(
                    config = move |c: &mut GridConfigurationScope| {
                        c.columns(three);
                        c.rows([GridTrackSize::Fixed(10.dp()); 2]);
                        c.set_flow(GridFlow::Column);
                        c.area("başlık", row = 1, column = 1, column_span = 3);
                        c.area("sağ", column = 3);
                    },
                    modifier = Modifier.test_tag("h"),
                    content = || {
                        Box(Modifier.fill_max_width().height(10.dp()).grid_item_area("başlık").test_tag("başlık"));
                        // Down the first column, under the header.
                        Box(Modifier.size(10.dp()).test_tag("s1"));
                        // The column is full: the next one.
                        Box(Modifier.size(10.dp()).test_tag("s2"));
                        // The last column's first free row.
                        Box(Modifier.size(10.dp()).grid_item_area("sağ").test_tag("sağ"));
                        // From the end.
                        Box(Modifier.size(10.dp()).grid_item(row = -1, column = -2).test_tag("son"));
                        Box(Modifier
                            .size(4.dp())
                            .grid_item_ranges(1..=2, 2..=3, alignment = Alignment::Center)
                            .test_tag("orta"));
                        Box(Modifier
                            .size(4.dp())
                            .grid_item(row = 2, column = 1, alignment = Alignment::BottomEnd)
                            .test_tag("köşe"));
                    },
                );
                // A row's first free column from where the last one placed by the flow ended.
                Grid(
                    config = |c: &mut GridConfigurationScope| c.columns([GridTrackSize::Fixed(10.dp()); 4]),
                    modifier = Modifier.test_tag("i"),
                    content = || {
                        Box(Modifier.size(10.dp()).grid_item(row = 1, column = 2));
                        // Past the one in the second column: a hole before it.
                        Box(Modifier.size(10.dp()).grid_item(column_span = 2));
                        Box(Modifier.size(10.dp()).grid_item(row = 1).test_tag("imleç"));
                    },
                );
            },
        );
    }));
    let cell = |grid, tag, row: i32, column: i32| at(&rule, grid, tag) == (column as f32 * 10.0, row as f32 * 10.0);
    assert!(cell("g", "a", 1, 2) && cell("g", "b", 0, 0) && cell("g", "c", 1, 0) && cell("g", "d", 2, 0));
    assert!(cell("g", "e", 2, 1) && cell("g", "f", 3, 0) && cell("g", "g1", 2, 2));
    assert!(near(bounds(&rule, "g")[3], 40.0));
    assert!(cell("h", "başlık", 0, 0) && near(bounds(&rule, "başlık")[2], 30.0));
    assert!(cell("h", "s1", 1, 0) && cell("h", "s2", 1, 1) && cell("h", "sağ", 1, 2));
    assert_eq!(at(&rule, "h", "son"), (10.0, 10.0));
    assert_eq!(at(&rule, "h", "orta"), (10.0 + 8.0, 8.0));
    assert_eq!(at(&rule, "h", "köşe"), (6.0, 16.0));
    assert!(cell("i", "imleç", 0, 4));
}

fn grid_takes_compose_arguments_and_its_config_scope() {
    // The Grid part of C++'s `layouts_take_compose_arguments_and_config_scopes` (kompoz_tests.cpp 11896):
    // `column(50)`, `column(1.fr)`, `gap(row = 4, column = 10)`, `gridItem(row = 1, column = 2)`.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Grid(
                    config = |c: &mut GridConfigurationScope| {
                        c.column(50.dp());
                        c.column(1.fr());
                        c.gap_rc(4.dp(), 10.dp());
                    },
                    modifier = Modifier.width(200.dp()),
                    content = || {
                        Box(Modifier.height(20.dp()).fill_max_width().grid_item(row = 1, column = 2).test_tag("hücre"));
                    },
                );
            },
        );
    }));
    let cell = bounds(&rule, "hücre");
    assert!(near(cell[0], 60.0) && near(cell[2], 140.0));
}

// ---- Compose's behaviours the C++ tests don't reach -------------------------------------------------------------

fn named_areas_span_stack_and_flow_along_one_dimensional_ones() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Grid(
            config = |c: &mut GridConfigurationScope| {
                c.columns([GridTrackSize::Fixed(20.dp()); 3]);
                c.rows([GridTrackSize::Fixed(10.dp()); 3]);
                // A 2D area across the first row; a column's two last rows; the last row, along which items flow.
                c.area("başlık", row = 1, column = 1, column_span = 3);
                c.area_ranges("kenar", 2..=3, 1..=1);
                c.area("şerit", row = 3);
                // Named again: the last one counts.
                c.area("taşınan", row = 1, column = 1);
                c.area("taşınan", row = 2, column = 3);
            },
            modifier = Modifier.test_tag("ızgara"),
            content = || {
                Box(Modifier.fill_max_width().height(10.dp()).grid_item_area("başlık").test_tag("üst1"));
                Box(Modifier.size(5.dp()).grid_item_area("başlık", alignment = Alignment::BottomEnd).test_tag("üst2"));
                Box(Modifier.fill_max_size().grid_item_area("kenar").test_tag("kenar"));
                Box(Modifier.size(10.dp()).grid_item_area("şerit").test_tag("ş1"));
                Box(Modifier.size(10.dp()).grid_item_area("şerit").test_tag("ş2"));
                Box(Modifier.size(10.dp()).grid_item_area("taşınan").test_tag("taşınan"));
                // The flow's cursor hasn't moved: the first free cell from the top start.
                Box(Modifier.size(10.dp()).test_tag("akan"));
                // A String isn't a &str: an area no one named, placed by the flow from its cursor (after "akan"),
                // in an implicit row: the explicit cells are taken.
                Box(Modifier.size(10.dp()).grid_item_area(String::from("başlık")).test_tag("başka"));
            },
        );
    }));
    let size = |tag| {
        let b = bounds(&rule, tag);
        (b[2], b[3])
    };
    assert_eq!(at(&rule, "ızgara", "üst1"), (0.0, 0.0));
    assert_eq!(size("üst1"), (60.0, 10.0));
    // One over the other in the same area, each by its alignment.
    assert_eq!(at(&rule, "ızgara", "üst2"), (55.0, 5.0));
    assert_eq!((at(&rule, "ızgara", "kenar"), size("kenar")), ((0.0, 10.0), (20.0, 20.0)));
    assert_eq!(at(&rule, "ızgara", "ş1"), (20.0, 20.0));
    assert_eq!(at(&rule, "ızgara", "ş2"), (40.0, 20.0));
    assert_eq!(at(&rule, "ızgara", "taşınan"), (40.0, 10.0));
    assert_eq!(at(&rule, "ızgara", "akan"), (20.0, 10.0));
    assert_eq!(at(&rule, "ızgara", "başka"), (0.0, 30.0));
}

fn negative_and_past_the_end_indices_and_implicit_tracks() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Grid(
                    config = |c: &mut GridConfigurationScope| {
                        c.columns([GridTrackSize::Fixed(10.dp()); 3]);
                        c.rows([GridTrackSize::Fixed(10.dp()); 2]);
                        c.gap(5.dp());
                    },
                    modifier = Modifier.test_tag("g"),
                    content = || {
                        // The last explicit row and column.
                        Box(Modifier.size(10.dp()).grid_item(row = -1, column = -1).test_tag("son"));
                        // Before the first row: left to the flow.
                        Box(Modifier.size(10.dp()).grid_item(row = -3).test_tag("akış"));
                        // Past the explicit columns: implicit Auto ones (the empty one 0 wide), their gaps counted.
                        Box(Modifier.size_wh(10.dp(), 7.dp()).grid_item(row = -2, column = 5).test_tag("dış"));
                        // Past the explicit rows too.
                        Box(Modifier.size_wh(8.dp(), 12.dp()).grid_item(row = 4, column = 1).test_tag("alt"));
                    },
                );
            },
        );
    }));
    assert_eq!(at(&rule, "g", "son"), (30.0, 15.0));
    assert_eq!(at(&rule, "g", "akış"), (0.0, 0.0));
    // Columns 10, 10, 10, 0, 10 with 4 gaps of 5; rows 10, 10, 0, 12 with 3.
    assert_eq!(at(&rule, "g", "dış"), (50.0, 0.0));
    assert_eq!(at(&rule, "g", "alt"), (0.0, 35.0));
    let g = bounds(&rule, "g");
    assert_eq!((g[2], g[3]), (60.0, 47.0));
}

fn column_flow_fills_columns_and_grows_implicit_ones() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Grid(
                    config = |c: &mut GridConfigurationScope| {
                        c.rows([GridTrackSize::Fixed(10.dp()); 2]);
                        c.set_flow(GridFlow::Column);
                        c.column_gap(5.dp());
                    },
                    modifier = Modifier.test_tag("g"),
                    content = || {
                        for i in 0..5 {
                            Box(Modifier.size(10.dp()).test_tag(format!("k{i}")));
                        }
                        // Three rows where there are two: not at a column's top, it wraps to the next column's,
                        // where it makes an implicit row (Auto, grown for it: Fixed rows don't grow).
                        Box(Modifier.size_wh(10.dp(), 50.dp()).grid_item(row_span = 3).test_tag("uzun"));
                    },
                );
            },
        );
    }));
    let expected = [(0.0, 0.0), (0.0, 10.0), (15.0, 0.0), (15.0, 10.0), (30.0, 0.0)];
    for (i, e) in expected.iter().enumerate() {
        assert_eq!(at(&rule, "g", &format!("k{i}")), *e, "k{i}");
    }
    assert_eq!(at(&rule, "g", "uzun"), (45.0, 0.0));
    let g = bounds(&rule, "g");
    assert_eq!((g[2], g[3]), (55.0, 50.0));
}

fn right_to_left_aligns_in_cells_without_mirroring_the_tracks() {
    // Compose places the items with `place`, not `placeRelative`: the first column stays on the left, an item's
    // `Start` alignment is its cell's right.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        KompozitionLocalProvider(&[LocalLayoutDirection.provides(LayoutDirection::Rtl)], || {
            Grid(
                config = |c: &mut GridConfigurationScope| c.columns([GridTrackSize::Fixed(50.dp()); 2]),
                modifier = Modifier.test_tag("g"),
                content = || {
                    Box(Modifier.size(10.dp()).test_tag("bir"));
                    Box(Modifier.size(10.dp()).test_tag("iki"));
                    Box(Modifier
                        .size(10.dp())
                        .grid_item(row = 2, column = 1, alignment = Alignment::Center)
                        .test_tag("orta"));
                    Box(Modifier
                        .size(10.dp())
                        .grid_item(row = 2, column = 2, alignment = AbsoluteAlignment::TopLeft)
                        .test_tag("sol"));
                },
            );
        });
    }));
    assert_eq!(at(&rule, "g", "bir"), (40.0, 0.0));
    assert_eq!(at(&rule, "g", "iki"), (90.0, 0.0));
    assert_eq!(at(&rule, "g", "orta"), (20.0, 10.0));
    assert_eq!(at(&rule, "g", "sol"), (50.0, 10.0));
}

fn intrinsics_are_compose_s_defaults() {
    // The Grid has the default intrinsics: measured at an unbounded width, where an Auto column is its max content,
    // so its min intrinsic width is too; a MinContent column is its min content either way.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Grid(
                    config = |c: &mut GridConfigurationScope| {
                        c.columns([GridTrackSize::Auto, GridTrackSize::Auto]);
                        c.column_gap(10.dp());
                    },
                    modifier = Modifier.width_intrinsic(IntrinsicSize::Min).test_tag("oto"),
                    content = || {
                        GridLeaf(20, 30, 300, Modifier.test_tag("o1"));
                        GridLeaf(10, 50, 500, Modifier.test_tag("o2"));
                    },
                );
                Grid(
                    config = |c: &mut GridConfigurationScope| {
                        c.columns([GridTrackSize::MinContent, GridTrackSize::MinContent]);
                        c.column_gap(10.dp());
                    },
                    modifier = Modifier.width_intrinsic(IntrinsicSize::Max).test_tag("en-az"),
                    content = || {
                        GridLeaf(20, 30, 300, Modifier);
                        GridLeaf(10, 50, 500, Modifier);
                    },
                );
                // Its rows' heights at its columns' widths: 300 / 30 and 1000 / 50.
                Grid(
                    config = |c: &mut GridConfigurationScope| {
                        c.columns([GridTrackSize::Fixed(30.dp()), GridTrackSize::Fixed(50.dp())])
                    },
                    modifier = Modifier.height_intrinsic(IntrinsicSize::Max).test_tag("yükseklik"),
                    content = || {
                        GridLeaf(10, 100, 300, Modifier);
                        GridLeaf(10, 100, 1000, Modifier);
                    },
                );
            },
        );
    }));
    assert!(near(bounds(&rule, "oto")[2], 90.0));
    assert!(near(bounds(&rule, "o1")[2], 30.0) && near(bounds(&rule, "o2")[2], 50.0));
    assert!(near(bounds(&rule, "en-az")[2], 40.0));
    assert!(near(bounds(&rule, "yükseklik")[3], 20.0));
}

fn auto_tracks_take_their_max_content_before_flexible_ones_share_the_rest() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(
            content = || {
                Grid(
                    config = |c: &mut GridConfigurationScope| {
                        c.columns([GridTrackSize::Auto, GridTrackSize::Flex(1.fr()), c.minmax(20.dp(), 3.fr())]);
                    },
                    modifier = Modifier.width(400.dp()),
                    content = || {
                        GridLeaf(20, 100, 1000, Modifier.test_tag("oto"));
                        Box(Modifier.fill_max_width().height(10.dp()).test_tag("esnek"));
                        Box(Modifier.fill_max_width().height(10.dp()).test_tag("minmax"));
                    },
                );
                // An item across two columns is asked its height at their width and the gap between them.
                Grid(
                    config = |c: &mut GridConfigurationScope| {
                        c.columns([GridTrackSize::Fixed(50.dp()); 2]);
                        c.gap(20.dp());
                    },
                    modifier = Modifier.test_tag("sarılan"),
                    content = || {
                        GridLeaf(10, 400, 1200, Modifier.grid_item(column_span = 2).test_tag("metin"));
                    },
                );
                // Percentages of the room after the gaps; the last gap call wins, each way.
                Grid(
                    config = |c: &mut GridConfigurationScope| {
                        c.column(0.5);
                        c.column(0.5);
                        c.row(GridTrackSize::Percentage(0.25));
                        c.row(0.75);
                        c.gap(10.dp());
                        c.column_gap(4.dp());
                    },
                    modifier = Modifier.size_wh(104.dp(), 50.dp()).test_tag("yüzde"),
                    content = || {
                        Box(Modifier.fill_max_size().test_tag("y1"));
                        Box(Modifier.fill_max_size().test_tag("y2"));
                        Box(Modifier.fill_max_size().test_tag("y3"));
                    },
                );
            },
        );
    }));
    assert!(near(bounds(&rule, "oto")[2], 100.0));
    // The Auto one at its max content, the MinMax at its 20: 280 left, shared 1 to 3.
    let (flexible, minmax) = (bounds(&rule, "esnek"), bounds(&rule, "minmax"));
    assert!(near(flexible[0], 100.0) && near(flexible[2], 70.0));
    assert!(near(minmax[0], 170.0) && near(minmax[2], 230.0));
    assert!(near(bounds(&rule, "metin")[2], 120.0) && near(bounds(&rule, "metin")[3], 10.0));
    assert!(near(bounds(&rule, "sarılan")[3], 10.0));
    let size = |tag| {
        let b = bounds(&rule, tag);
        (b[2], b[3])
    };
    assert_eq!((size("y1"), size("y2"), size("y3")), ((50.0, 10.0), (50.0, 10.0), (50.0, 30.0)));
    assert_eq!(at(&rule, "yüzde", "y2"), (54.0, 0.0));
    assert_eq!(at(&rule, "yüzde", "y3"), (0.0, 20.0));
}

fn config_adapts_to_constraints_and_state_and_is_kept_while_the_same() {
    RUNS.with(|r| r.set(0));
    let tick = mutable_state_of(0);
    let count = mutable_state_of(2);
    let width = mutable_state_of(20.0f32);
    let (t, n, w) = (tick.clone(), count.clone(), width.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let _ = t.get();
        let n = n.get();
        let w = w.clone();
        Column(
            content = || {
                // A column for each, as wide as a state read as it measures says.
                Grid(
                    config = move |c: &mut GridConfigurationScope| {
                        RUNS.with(|r| r.set(r.get() + 1));
                        for _ in 0..n {
                            c.column(Dp(w.get()));
                        }
                    },
                    modifier = Modifier.test_tag("sayan"),
                    content = || Box(Modifier.size(5.dp())),
                );
                // By its constraints: one column when narrow.
                for width in [200, 400] {
                    Box(
                        Modifier.width(Dp(width as f32)),
                        content = || {
                            Grid(
                                config = |c: &mut GridConfigurationScope| {
                                    assert_eq!(c.flow(), GridFlow::Row);
                                    let columns = if c.constraints().max_width < 300 { 1 } else { 2 };
                                    for _ in 0..columns {
                                        c.column(1.fr());
                                    }
                                    assert!(c.density == 1.0 && c.round_to_px(10.dp()) == 10);
                                },
                                content = || {
                                    Box(Modifier.height(10.dp()).fill_max_width().test_tag(format!("bir{width}")));
                                    Box(Modifier.height(10.dp()).fill_max_width().test_tag(format!("iki{width}")));
                                },
                            );
                        },
                    );
                }
            },
        );
    }));
    rule.set_layout_check(false);
    assert!(near(bounds(&rule, "sayan")[2], 40.0));
    assert_eq!(at(&rule, "bir200", "iki200"), (0.0, 10.0));
    assert!(near(bounds(&rule, "iki200")[2], 200.0));
    assert_eq!(at(&rule, "bir400", "iki400"), (200.0, 0.0));
    let before = runs();
    assert!(before >= 1);
    // Composed again with the same config: not measured again.
    tick.set(1);
    rule.wait_for_idle();
    assert_eq!(runs(), before);
    // A state it read changed: measured again, with what it says now.
    width.set(30.0);
    rule.wait_for_idle();
    assert_eq!(runs(), before + 1);
    assert!(near(bounds(&rule, "sayan")[2], 60.0));
    // Another config (another value captured): a new policy, measured again.
    count.set(3);
    rule.wait_for_idle();
    assert_eq!(runs(), before + 2);
    assert!(near(bounds(&rule, "sayan")[2], 90.0));
}

fn track_sizes_flows_and_weights_write_and_compare_as_compose() {
    use GridTrackSize as T;
    assert_eq!(format!("{:?}", T::Fixed(10.dp())), "Fixed(10.0dp)");
    assert_eq!(format!("{:?}", T::Percentage(0.5)), "Percentage(0.5)");
    assert_eq!(format!("{:?}", T::Flex(2.fr())), "Flex(2.0fr)");
    assert_eq!(format!("{:?}", T::MinMax(40.dp(), 1.fr())), "MinMax(40.0dp, 1.0fr)");
    // A MinMax keeps its values' top 30 bits, as Compose packs them.
    assert_eq!(format!("{:?}", T::MinMax(Dp(0.1), 1.fr())), "MinMax(0.099999994dp, 1.0fr)");
    assert_eq!(format!("{:?} {:?} {:?}", T::MinContent, T::MaxContent, T::Auto), "MinContent MaxContent Auto");
    assert_eq!(format!("{:?} {:?}", GridFlow::Row, GridFlow::Column), "Row Column");
    assert_eq!(format!("{:?}", 1.fr()), "1.0.fr");
    assert_eq!(format!("{:?}", 0.5.fr()), "0.5.fr");
    assert!(1.fr() == 1.0f32.fr() && 1.fr() == 1.0f64.fr() && 1.fr().value() == 1.0 && Fr(f32::NAN) == Fr(f32::NAN));
    assert!(T::Fixed(10.dp()) == T::Fixed(10.dp()) && T::Fixed(10.dp()) != T::Flex(Fr(10.0)));
    assert!(T::Percentage(0.0) != T::Fixed(0.dp()) && T::Auto != T::MinContent);
    assert_eq!((GridScope::MaxGridIndex, GridScope::GridIndexUnspecified), (1000, 0));
    // Compose's requires.
    let panics = |f: fn()| std::panic::catch_unwind(f).is_err();
    let track_panics = |f: fn() -> GridTrackSize| std::panic::catch_unwind(f).is_err();
    assert!(track_panics(|| T::Fixed(Dp::Unspecified)));
    assert!(track_panics(|| T::Fixed((-1).dp())));
    assert!(track_panics(|| T::Percentage(-0.1)));
    assert!(track_panics(|| T::Flex(Fr(-1.0))));
    assert!(track_panics(|| T::MinMax((-1).dp(), 1.fr())));
    assert!(track_panics(|| T::MinMax(0.dp(), Fr(-1.0))));
    assert!(!track_panics(|| T::Fixed(Dp::Infinity)));
    assert!(panics(|| drop(Modifier.grid_item_with(1001, 0, 1, 1, Alignment::TopStart))));
    assert!(panics(|| drop(Modifier.grid_item_with(0, -1001, 1, 1, Alignment::TopStart))));
    assert!(panics(|| drop(Modifier.grid_item_with(0, 0, 0, 1, Alignment::TopStart))));
    assert!(panics(|| drop(Modifier.grid_item_with(0, 0, 1, 0, Alignment::TopStart))));
    #[allow(clippy::reversed_empty_ranges)]
    let empty = || drop(Modifier.grid_item_ranges(3..=2, 1..=1));
    assert!(panics(empty));
    assert!(!panics(|| drop(Modifier.grid_item_with(-1000, 1000, 1, 1, Alignment::TopStart))));
    assert!(!panics(|| drop(Modifier.grid_item())));
    assert_eq!(format!("{:?}", Modifier.grid_item_area("a")), "Modifier.gridItem");
    // Equal arguments, equal elements (the area's id by its value).
    assert!(Modifier.grid_item_area("a") == Modifier.grid_item_area("a"));
    assert!(Modifier.grid_item_area("a") != Modifier.grid_item_area(String::from("a")));
    assert!(
        Modifier.grid_item_ranges_with(2..=3, 1..=2, Alignment::Center)
            == Modifier.grid_item_with(2, 1, 2, 2, Alignment::Center)
    );
}

tests!(
    grid_sizes_tracks_as_compose,
    grid_places_items_as_compose,
    grid_takes_compose_arguments_and_its_config_scope,
    named_areas_span_stack_and_flow_along_one_dimensional_ones,
    negative_and_past_the_end_indices_and_implicit_tracks,
    column_flow_fills_columns_and_grows_implicit_ones,
    right_to_left_aligns_in_cells_without_mirroring_the_tracks,
    intrinsics_are_compose_s_defaults,
    auto_tracks_take_their_max_content_before_flexible_ones_share_the_rest,
    config_adapts_to_constraints_and_state_and_is_kept_while_the_same,
    track_sizes_flows_and_weights_write_and_compare_as_compose,
);
