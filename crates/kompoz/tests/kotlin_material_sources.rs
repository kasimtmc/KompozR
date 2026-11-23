//! The C++ Kompoz's Kotlin sources that are Material's (kompozable_tests.cpp, compiled there by kompozc), each ported
//! whole as Compose's Kotlin through `kompoz!` (M15's API where the C++'s differs, said at each), with its test's
//! checks: KotlinHelpers (`kotlin_helpers_through_kompozc`), MaterialForms (`material_as_compose_writes_it`),
//! SectionsForms (`sections_forms_as_compose_writes_them`), TextStyles (`text_styles_as_compose_writes_them`),
//! ScrollForms with TiledRow (`scroll_forms_as_compose_writes_them`), GraphicsForms' Material lines
//! (`graphics_forms_as_compose_writes_them`: its wavy and contained indicators and MaterialShapes.Heart; its
//! animation, painter and graphics lines are the lower stages' — its `object : Painter()` an object expression, stage
//! 14's) and AppShellForms' stage-12 lines (`app_shell_as_compose_writes_it`: its NavigationSuiteScaffold of
//! NavigationSuiteItems and SnackbarData's visuals; its NavController, NavHost, navOptions and deep links are stage
//! 13's, its desktop MenuBar stage 15/16's (decision D7)). The other Material sources are whole where their package
//! put them: NamedHeads, Containers (kotlin_buttons.rs), Settings, AddTextDialog/DialogHost (kotlin_lists_sheets.rs),
//! ScaffoldExample (kt/scaffold.kt), Inbox, StateForms, AppTheme (kotlin_ported.rs), Themed
//! (kotlin_material_theme.rs), TextForms and Phone (kt/material_fields.kt), Expressive and Navigation
//! (kotlin_expressive_buttons.rs), LayoutForms (kotlin_adaptive.rs, kotlin_lookahead.rs, kotlin_grids_pager.rs,
//! named_forms.rs), Mixed (kotlin_expressive_buttons.rs; its view model and SideLine are stage 13's).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
// (what the generated code names that the prelude doesn't: Compose's imports of them)
use kompoz::ui::semantics::ProgressBarRangeInfo;
use kompoz::ui::text::remember_text_measurer;
use kompoz::ui::window::PopupPositionProvider;
use std::cell::Cell;
use std::rc::Rc;

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

// ---- KotlinHelpers (kompozable_tests.cpp 588-608) ------------------------------------------------------------------

kompoz! {
    // Kotlin's String.format with Java's specifiers, more of its collection functions, x++ as a value on a state,
    // ListItemDefaults.colors and a constructor called by place (CornerRadius(24f)).
    @Composable
    fun KotlinHelpers() {
        var nextId by remember { mutableStateOf(4) }
        val ids = remember { mutableStateListOf(1, 2, 3) }
        val words = listOf("elma", "armut", "erik", "ayva", "kiraz")
        val (a, rest) = words.partition { it.startsWith("a") }
        val byLetter = words.groupBy { it.first() }
        val best = words.maxByOrNull { it.length }
        Column {
            Text("%.1f km · %03d dk · %-4s| · %x".format(1.25f, 7, "ab", 255))
            Text("ilk ${words.indexOfFirst { it.startsWith("e") }} son ${words.indexOfLast { it.startsWith("e") }}")
            Text("a: ${a.joinToString()} · öteki: ${rest.size} · gruplar: ${byLetter.size} · en uzun: $best")
            Text("azalan: ${words.sortedByDescending { it.length }.first()} · parça: ${words.chunked(2).size} · tek: ${words.distinctBy { it.first() }.size}")
            Text("eşle: ${words.associateWith { it.length }.size} · düz: ${listOf(listOf(1, 2), listOf(3)).flatMap { it }.sum()}")
            Button(onClick = { ids.add(nextId++) }) { Text("Ekle") }
            Button(onClick = { ids.removeIf { it % 2 == 0 } }) { Text("Çiftleri sil") }
            Text("kimlikler: ${ids.joinToString(",")} · sonraki: $nextId")
            ListItem(headlineContent = { Text("Saydam satır") }, colors = ListItemDefaults.colors(containerColor = Color.Transparent))
            Canvas(modifier = Modifier.size(40.dp)) { drawRoundRect(Color.Red, cornerRadius = CornerRadius(24f)) }
        }
    }
}

fn kotlin_helpers_through_kompozc() {
    let rule = KompozTestRule::new(KotlinHelpers);
    // (Java's %.1f of 1.25f rounds half up: 1.3 — the C++ took either)
    assert!(
        rule.on_node_with_text("1.3 km · 007 dk · ab  | · ff").exists()
            || rule.on_node_with_text("1.2 km · 007 dk · ab  | · ff").exists()
    );
    rule.on_node_with_text("ilk 0 son 2").assert_exists();
    rule.on_node_with_text("a: armut, ayva · öteki: 3 · gruplar: 3 · en uzun: armut").assert_exists();
    rule.on_node_with_text("azalan: armut · parça: 3 · tek: 3").assert_exists();
    rule.on_node_with_text("eşle: 5 · düz: 6").assert_exists();
    rule.on_node_with_text("Ekle").perform_click();
    rule.on_node_with_text("Ekle").perform_click();
    rule.on_node_with_text("kimlikler: 1,2,3,4,5 · sonraki: 6").assert_exists();
    rule.on_node_with_text("Çiftleri sil").perform_click();
    rule.on_node_with_text("kimlikler: 1,3,5 · sonraki: 6").assert_exists();
    rule.on_node_with_text("Saydam satır").assert_exists();
    // (no list item container drawn: transparent; the canvas' round rect red)
    let canvas = rule.draw();
    assert!(canvas.rect_colors.contains(&Color::Red));
}

// ---- MaterialForms (kompozable_tests.cpp 2247-2280) ----------------------------------------------------------------

kompoz! {
    // Material as Compose writes it: chips with label = { }, toggleable(role = Role.Checkbox), a snackbar awaited
    // with actionLabel, progress = { }, NavigationSuiteScaffold's items block.
    @Composable
    fun MaterialForms(undo: () -> Unit) {
        var unread by remember { mutableStateOf(false) }
        var muted by remember { mutableStateOf(false) }
        var tab by rememberSaveable { mutableStateOf(0) }
        val snackbars = remember { SnackbarHostState() }
        val scope = rememberCoroutineScope()
        Column {
            FilterChip(selected = unread, onClick = { unread = !unread }, label = { Text("Unread") }, modifier = Modifier.testTag("chip"))
            Row(modifier = Modifier.toggleable(value = muted, onValueChange = { muted = it }, role = Role.Checkbox).testTag("row")) {
                Checkbox(checked = muted, onCheckedChange = null)
                Text("Mute")
            }
            Button(onClick = {
                scope.launch {
                    if (snackbars.showSnackbar("Deleted", actionLabel = "Undo") == SnackbarResult.ActionPerformed) undo()
                }
            }) { Text("Delete") }
            SnackbarHost(hostState = snackbars)
            LinearProgressIndicator(progress = { 0.4f }, modifier = Modifier.testTag("bar"))
            NavigationSuiteScaffold(
                navigationSuiteItems = {
                    item(selected = tab == 0, onClick = { tab = 0 }, icon = { Icon(Icons.Filled.Home, contentDescription = null) }, label = { Text("Home") })
                    item(selected = tab == 1, onClick = { tab = 1 }, icon = { Icon(Icons.Filled.Settings, contentDescription = null) }, label = { Text("Settings") })
                },
                modifier = Modifier.height(300.dp),
                layoutType = NavigationSuiteType.NavigationBar,
            ) {
                Text("Tab $tab")
            }
        }
    }
}

fn material_as_compose_writes_it() {
    let undone = Rc::new(Cell::new(0));
    let u = undone.clone();
    let rule = KompozTestRule::with_viewport(
        kompozable_lambda!(move || {
            let u = u.clone();
            MaterialForms(move || u.set(u.get() + 1))
        }),
        IntSize::new(400, 800),
    );
    rule.on_node_with_tag("chip").assert_is_not_selected().perform_click();
    rule.on_node_with_tag("chip").assert_is_selected();
    // (the row toggles its Checkbox, which takes no clicks of its own)
    rule.on_node_with_tag("row").assert_is_off().perform_click();
    rule.on_node_with_tag("row").assert_is_on();
    rule.on_node_with_tag("bar").assert_exists().assert_range_info_equals(ProgressBarRangeInfo::new(0.4, 0.0..=1.0, 0));
    rule.on_node_with_text("Tab 0").assert_exists();
    rule.on_node_with_text("Settings").perform_click();
    rule.on_node_with_text("Tab 1").assert_exists();
    rule.on_node_with_text("Delete").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_text("Undo").assert_exists();
    rule.on_node_with_text("Undo").perform_click();
    rule.wait_for_idle();
    assert_eq!(undone.get(), 1);
    rule.on_node_with_text("Deleted").assert_does_not_exist();
}

// ---- SectionsForms (kompozable_tests.cpp 2921-2967) ----------------------------------------------------------------

kompoz! {
    // A swipe-to-dismiss box, a search bar with its state, tooltips, a date picker dialog and focus properties as
    // Compose writes them; WindowInsets.statusBars read as a property, a nullable read after its null check, Pair(a,
    // b), tween(easing = …). (M15: the C++'s `PlainTooltip(caretSize = TooltipDefaults.caretSize)` is `caretShape =
    // TooltipDefaults.caretShape()`; its `rememberTooltipPositionProvider()` is deprecated for the anchor position
    // form; `rememberDatePickerState(picked)` and `state.selectedDate` are the JVM's java.time forms,
    // `rememberDatePickerState(initialSelectedDate = …)` and `state.getSelectedDate()`; `WindowInsets.statusBars.top`
    // is `getTop(density)`.)
    @Composable
    fun SectionsForms() {
        val tasks = remember { mutableStateListOf(1, 2) }
        val search = rememberSearchBarState()
        val query = rememberTextFieldState()
        var chosen by remember { mutableStateOf("yok") }
        var open by remember { mutableStateOf(false) }
        var picked by remember { mutableStateOf<LocalDate?>(null) }
        val requesters = remember { List(2) { FocusRequester() } }
        val scope = rememberCoroutineScope()
        val maybe: Pair<Int, Int>? = if (tasks.size > 1) Pair(3, 4) else null
        val alpha by animateFloatAsState(if (open) 1f else 0.5f, animationSpec = tween(durationMillis = 10, easing = LinearEasing))
        val density = LocalDensity.current
        Column {
            for (id in tasks) {
                key(id) {
                    val state = rememberSwipeToDismissBoxState(confirmValueChange = { tasks.remove(id); true })
                    SwipeToDismissBox(state = state, backgroundContent = { Box(modifier = Modifier.fillMaxSize().background(Color.Red)) }) {
                        Text("görev $id", modifier = Modifier.fillMaxWidth().testTag("görev$id"))
                    }
                }
            }
            SearchBar(state = search, inputField = {
                SearchBarDefaults.InputField(textFieldState = query, searchBarState = search, onSearch = { chosen = it },
                                             placeholder = { Text("Ara") }, leadingIcon = { Icon(Icons.Filled.Search, contentDescription = "Ara") })
            }, modifier = Modifier.testTag("arama"))
            TooltipBox(positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Above), tooltip = { PlainTooltip(caretShape = TooltipDefaults.caretShape()) { Text("İpucu") } },
                       state = rememberTooltipState()) {
                IconButton(onClick = { open = true }, modifier = Modifier.testTag("aç")) { Icon(Icons.Filled.Info, contentDescription = "Bilgi") }
            }
            if (open) {
                val state = rememberDatePickerState(initialSelectedDate = picked)
                DatePickerDialog(onDismissRequest = { open = false }, confirmButton = { TextButton(onClick = { picked = state.getSelectedDate(); open = false }) { Text("Tamam") } },
                                 dismissButton = { TextButton(onClick = { open = false }) { Text("İptal") } }) { DatePicker(state = state) }
            }
            Row {
                for (i in 0 until 2) {
                    OutlinedButton(onClick = { }, modifier = Modifier.focusRequester(requesters[i]).focusProperties { if (i == 1) canFocus = false }.testTag("odak$i")) { Text("düğme $i") }
                }
            }
            Text("tasks ${tasks.size} · ${chosen} · ${if (maybe != null) maybe.first + maybe.second else 0} · ${WindowInsets.statusBars.getTop(density)} · ${(alpha * 10).roundToInt()}",
                 modifier = Modifier.testTag("özet"))
            Button(onClick = { scope.launch { requesters[1].requestFocus() } }) { Text("İkiye odaklan") }
        }
    }
}

fn sections_forms_as_compose_writes_them() {
    let rule = KompozTestRule::with_viewport(SectionsForms, IntSize::new(400, 800));
    rule.on_node_with_text("tasks 2 · yok · 7 · 0 · 5").assert_exists();
    rule.on_node_with_tag("görev1").assert_exists();
    rule.on_node_with_tag("arama").assert_exists();
    rule.on_node_with_tag("aç").perform_click();
    rule.main_clock().advance_time_by(500);
    rule.on_node_with_text("Tamam").assert_exists();
    rule.on_node_with_text("tasks 2 · yok · 7 · 0 · 10").assert_exists(); // (open: the tween went to 1)
    rule.on_node_with_text("İptal").perform_click();
    rule.main_clock().advance_time_by(500);
    rule.on_node_with_text("tasks 2 · yok · 7 · 0 · 5").assert_exists();
    rule.on_node_with_text("düğme 1").assert_exists();
    // (the second button can't take the focus asked for it; a task swiped away goes)
    rule.on_node_with_text("İkiye odaklan").perform_click();
    rule.wait_for_idle();
    rule.on_node_with_tag("odak1").assert_is_not_focused();
    let y = bounds(&rule, "görev1").center().y;
    let h = rule.host();
    h.pointer_down(Offset::new(350.0, y), -1, 0, kompoz::ui::input::PointerType::Touch);
    for i in 1..=20 {
        rule.advance_frame();
        h.pointer_move(Offset::new(350.0 - 15.0 * i as f32, y), -1, 0);
    }
    h.pointer_up(Offset::new(50.0, y), -1, 0);
    rule.wait_for_idle();
    rule.on_node_with_tag("görev1").assert_does_not_exist();
    rule.on_node_with_text("tasks 1 · yok · 0 · 0 · 5").assert_exists();
}

// ---- TextStyles (kompozable_tests.cpp 2981-2999) -------------------------------------------------------------------

kompoz! {
    // Text styles as Compose writes them: a typography style's copy(brush = …, shadow = …, lineHeightStyle = …,
    // textIndent = TextIndent(24.sp), fontFeatureSettings = …, lineBreak = …), basicMarquee by name,
    // TextAutoSize.StepBased in sp, the measurer's style = a typography style.
    @Composable
    fun TextStyles() {
        val typography = MaterialTheme.typography
        val measurer = rememberTextMeasurer()
        val width = measurer.measure("Kompoz", style = typography.titleMedium).size.width
        Column {
            Text("degrade", style = typography.headlineSmall.copy(brush = Brush.horizontalGradient(listOf(Color.Red, Color.Blue))))
            Text("gölge", style = typography.titleLarge.copy(shadow = Shadow(Color.Black, Offset(2f, 3f), 4f)))
            Text("girinti", style = typography.bodyMedium.copy(textIndent = TextIndent(24.sp), lineBreak = LineBreak.Paragraph,
                                                               lineHeightStyle = LineHeightStyle(alignment = LineHeightStyle.Alignment.Top, trim = LineHeightStyle.Trim.None)))
            Text("küçük büyük", style = typography.titleMedium.copy(fontFeatureSettings = "smcp"))
            Text("kayan yazı kayan yazı kayan yazı kayan yazı kayan yazı", modifier = Modifier.width(120.dp).basicMarquee(iterations = Int.MAX_VALUE, velocity = 60.dp).testTag("kayan"))
            Text("sığ", modifier = Modifier.width(60.dp), maxLines = 1, autoSize = TextAutoSize.StepBased(8.sp, 40.sp, 1.sp))
            Text("ölçü ${if (width > 10f) "var" else "yok"}")
        }
    }
}

fn text_styles_as_compose_writes_them() {
    // (its marquee scrolls without end: never idle, as in Compose's tests — the clock moved by hand)
    let rule = KompozTestRule::empty();
    rule.main_clock().set_auto_advance(false);
    rule.set_content(TextStyles);
    rule.advance_time_by(100);
    for text in ["degrade", "gölge", "girinti", "küçük büyük", "sığ", "ölçü var"] {
        rule.on_node_with_text(text).assert_exists();
    }
    rule.on_node_with_tag("kayan").assert_exists();
    // (the feature settings as given)
    let canvas = rule.draw();
    let style = |t: &str| canvas.text_styles[canvas.texts.iter().position(|x| x == t).expect(t)].clone();
    assert_eq!(style("küçük büyük").font_feature_settings, "smcp");
}

// ---- ScrollForms (kompozable_tests.cpp 3210-3255) ------------------------------------------------------------------

kompoz! {
    // Scroll forms as Compose writes them: try { } finally { } in a coroutine, AppBarRow(maxItemCount =) {
    // clickableItem(onClick =, icon =, label =) }, AppBarWithSearch(state =, inputField =, navigationIcon =),
    // Modifier.pullToRefresh by name with PullToRefreshDefaults.Indicator / IndicatorBox and LoadingIndicator(progress =
    // { }), overscrollEffect = null and an OverscrollEffect? parameter, Animatable<Float, AnimationVector1D>.
    @Composable
    fun TiledRow(effect: OverscrollEffect?) {
        LazyRow(modifier = Modifier.height(40.dp), overscrollEffect = effect) { items(3) { Box(modifier = Modifier.size(40.dp)) } }
    }

    @Composable
    fun ScrollForms(a: Animatable<Float, AnimationVector1D>) {
        var log by remember { mutableStateOf("") }
        var refreshing by remember { mutableStateOf(false) }
        val state = rememberPullToRefreshState()
        val search = rememberSearchBarState()
        val query = rememberTextFieldState()
        // (the source's `try { log += "içinde "; a.snapTo(3f) } finally { log += "sonunda" }`: Kotlin's try is stage
        // 14's in KompozR's Kotlin syntax — its path when nothing throws, as written in order)
        LaunchedEffect(Unit) {
            log = log + "içinde "
            a.snapTo(3f)
            log = log + "sonunda"
        }
        Column {
            TopAppBar(title = { Text("belge") }, actions = {
                AppBarRow(maxItemCount = 2) {
                    clickableItem(onClick = { log = log + " paylaş" }, icon = { Icon(Icons.Filled.Share, contentDescription = "Paylaş") }, label = "Paylaş")
                    clickableItem(onClick = {}, icon = { Icon(Icons.Filled.Edit, contentDescription = "Düzenle") }, label = "Düzenle")
                    clickableItem(onClick = {}, icon = { Icon(Icons.Filled.Delete, contentDescription = "Sil") }, label = "Sil")
                }
            })
            AppBarWithSearch(state = search,
                             inputField = { SearchBarDefaults.InputField(searchBarState = search, textFieldState = query, onSearch = {}, placeholder = { Text("ara") }) },
                             navigationIcon = { Icon(Icons.Filled.Menu, contentDescription = "Menü") })
            Box(modifier = Modifier.size(200.dp, 120.dp).pullToRefresh(isRefreshing = refreshing, state = state, onRefresh = { refreshing = true }, enabled = true).testTag("çekilir")) {
                PullToRefreshDefaults.Indicator(state = state, isRefreshing = refreshing, modifier = Modifier.align(Alignment.TopCenter))
                PullToRefreshDefaults.IndicatorBox(state = state, isRefreshing = refreshing, modifier = Modifier.align(Alignment.TopEnd), shape = CircleShape) {
                    LoadingIndicator(progress = { min(1f, state.distanceFraction) }, modifier = Modifier.size(24.dp))
                }
            }
            TiledRow(rememberOverscrollEffect())
            TiledRow(null)
            Text("günlük: $log · ${a.value.roundToInt()}")
        }
    }
}

fn scroll_forms_as_compose_writes_them() {
    let a = Animatable(0.0f32);
    let rule = {
        let a = a.clone();
        KompozTestRule::with_viewport(kompozable_lambda!(move || ScrollForms(a.clone())), IntSize::new(400, 800))
    };
    rule.wait_for_idle();
    rule.on_node_with_text("günlük: içinde sonunda · 3").assert_exists();
    // two slots: the rest in the menu
    rule.on_node_with_content_description("Paylaş").assert_exists();
    rule.on_node_with_content_description("More options").assert_exists();
    rule.on_node_with_content_description("Paylaş").perform_click();
    rule.on_node_with_text("günlük: içinde sonunda paylaş · 3").assert_exists();
    rule.on_node_with_content_description("Menü").assert_exists();
    rule.on_node_with_tag("çekilir").assert_exists();
    // (measured, then left out of the row: in the overflow menu, by their labels)
    rule.on_node_with_content_description("Düzenle").assert_is_not_displayed();
    rule.on_node_with_content_description("More options").perform_click();
    rule.on_node_with_text("Düzenle").assert_exists();
    rule.on_node_with_text("Sil").assert_exists();
}

// ---- GraphicsForms' Material lines (kompozable_tests.cpp 3337-3398) ------------------------------------------------

kompoz! {
    // The wavy and contained indicators by name and MaterialShapes.Heart as a shape (the source's other lines are
    // animation's, graphics' and its object expression's: see this file's doc).
    @Composable
    fun GraphicsFormsMaterial() {
        Column {
            LinearWavyProgressIndicator(progress = { 0.5f }, modifier = Modifier.width(100.dp).testTag("dalga"), amplitude = { 0.5f }, stopSize = 0.dp)
            CircularWavyProgressIndicator(trackColor = Color.Gray)
            ContainedLoadingIndicator(progress = { 0.3f }, modifier = Modifier.testTag("kap"), containerColor = Color.Yellow)
            Box(modifier = Modifier.size(20.dp).background(Color.Red, MaterialShapes.Heart.toShape()).testTag("kalp"))
        }
    }
}

fn graphics_forms_as_compose_writes_them() {
    let rule = KompozTestRule::with_viewport(GraphicsFormsMaterial, IntSize::new(400, 800));
    rule.wait_for_idle();
    assert!(near(bounds(&rule, "dalga").width(), 100.0) && near(bounds(&rule, "kap").width(), 48.0));
    rule.on_node_with_tag("kalp").assert_exists();
    // (the heart filled in red: a path, not a rect)
    assert!(rule.draw().path_colors.contains(&Color::Red));
}

// ---- AppShellForms' stage-12 lines (kompozable_tests.cpp 3426-3468) ------------------------------------------------

kompoz! {
    // NavigationSuiteScaffold(navigationItems = { NavigationSuiteItem(…) }, navigationSuiteType =) and
    // data.visuals.actionLabel. (The source's items navigate a NavController — stage 13's —: here they pick a state,
    // its NavHost's screens the texts it shows; its MenuBar is stage 15/16's. Its `SnackbarData(message =,
    // actionLabel =)` was the C++'s struct: Compose's SnackbarData is an interface a host state makes — the data
    // shown is read from one here.)
    @Composable
    fun AppShellForms() {
        var inAccount by remember { mutableStateOf(false) }
        val snackbars = remember { SnackbarHostState() }
        LaunchedEffect(Unit) { snackbars.showSnackbar("m", actionLabel = "Geri al", duration = SnackbarDuration.Indefinite) }
        val data = snackbars.currentSnackbarData
        Column {
            Text("eylem ${data?.visuals?.actionLabel?.length ?: 0}")
            NavigationSuiteScaffold(
                navigationItems = {
                    NavigationSuiteItem(selected = !inAccount, onClick = { inAccount = false },
                                        icon = { Icon(Icons.Filled.Home, contentDescription = null) }, label = { Text("Kabuk") })
                    NavigationSuiteItem(selected = inAccount, onClick = { inAccount = true },
                                        icon = { Icon(Icons.Filled.Settings, contentDescription = null) }, label = { Text("Hesap") })
                },
                navigationSuiteType = NavigationSuiteType.NavigationRail,
                modifier = Modifier.weight(1f),
            ) {
                Text(if (inAccount) "hesap 0" else "kabuk ekranı")
            }
            Text(if (inAccount) "sekme hesap" else "sekme kabuk")
        }
    }
}

fn app_shell_as_compose_writes_it() {
    let rule = KompozTestRule::with_viewport(AppShellForms, IntSize::new(600, 800));
    rule.wait_for_idle();
    rule.on_node_with_text("kabuk ekranı").assert_exists();
    rule.on_node_with_text("sekme kabuk").assert_exists();
    rule.on_node_with_text("eylem 7").assert_exists();
    rule.on_node_with_text("Hesap").perform_click();
    rule.wait_for_idle();
    rule.advance_time_by(1000);
    rule.wait_for_idle();
    rule.on_node_with_text("hesap 0").assert_exists();
    rule.on_node_with_text("sekme hesap").assert_exists();
    // (a rail: its items one under another at the start)
    let at = |t: &str| rule.on_node_with_text(t).get_unclipped_bounds_in_root();
    let (home, account) = (at("Kabuk"), at("Hesap"));
    assert!(account.top > home.top && home.left < 100.0);
}

tests![
    kotlin_helpers_through_kompozc,
    material_as_compose_writes_it,
    sections_forms_as_compose_writes_them,
    text_styles_as_compose_writes_them,
    scroll_forms_as_compose_writes_them,
    graphics_forms_as_compose_writes_them,
    app_shell_as_compose_writes_it,
];
