//! (stage 13 WP2) Navigation's core in Kotlin's syntax, as a Compose user writes it with a NavController: typed
//! routes' `toRoute<T>()` and `hasRoute<T>()`, `NavDestination.hierarchy`, `navOptions { popUpTo<T> { saveState =
//! true } }`, `navDeepLink { uriPattern = … }` and `navDeepLink<T>(basePath)`, an entry's `arguments?.getInt("n")`,
//! `navigate(route) { … }` and `popBackStack<T>(inclusive = …)`. The controller's destinations are a test
//! navigator's here (the NavHost showing them: kotlin_nav_host.rs).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use std::rc::Rc;

kompoz! {
    // The C++ Kompoz's routes (kompozable_tests.cpp's Navigator's and AppShellForms').
    @Serializable
    object Shell

    @Serializable
    object Start

    @Serializable
    object AccountGraph

    @Serializable
    data class Member(val n: Int)

    @Serializable
    data class Detail(val id: Int, val tab: String? = null)

    @Kompozable
    fun NavForms(nav: NavController) {
        val destination = nav.currentDestination
        val inAccount = destination?.hierarchy?.any { it.hasRoute<AccountGraph>() } == true
        val tabs = navOptions {
            popUpTo<Shell> { saveState = true }
            launchSingleTop = true
            restoreState = true
        }
        val link = navDeepLink { uriPattern = "kompoz://a/{n}" }
        val member = navDeepLink<Member>("kompoz://member")
        val n = nav.currentBackStackEntry?.arguments?.getInt("n") ?: 0
        val entry = nav.currentBackStackEntry
        val detail = if (entry?.destination?.hasRoute<Detail>() == true) entry?.toRoute<Detail>() else null
        Column {
            Text("hesapta $inAccount n $n ayrıntı ${detail?.id ?: -1} ${link.uriPattern} ${member.uriPattern}")
            Button(onClick = { nav.navigate(AccountGraph, tabs) }) { Text("hesap") }
            Button(onClick = { nav.navigate(Shell, tabs) }) { Text("kabuk") }
            Button(onClick = {
                nav.navigate(Detail(7, "b")) {
                    popUpTo<Start>()
                    launchSingleTop = true
                }
            }) { Text("ayrıntı") }
            Button(onClick = { nav.popBackStack<AccountGraph>(inclusive = true) }) { Text("geri") }
            Button(onClick = { nav.navigate("a/3") { popUpTo("grup") { inclusive = true } } }) { Text("a") }
        }
    }
}

// ---- the controller (a test navigator's destinations) -------------------------------------------------------------

#[derive(Default)]
struct TestNavigator {
    base: NavigatorBase,
}

impl Navigator for TestNavigator {
    fn name(&self) -> String {
        "test".into()
    }
    fn base(&self) -> &NavigatorBase {
        &self.base
    }
    fn create_destination(&self) -> NavDestination {
        NavDestination::new("test")
    }
}

fn controller() -> NavController {
    let nav = NavController::new();
    nav.navigator_provider().add_navigator(Rc::new(TestNavigator::default()));
    let graph = nav.create_graph_typed(Shell {}, None, TypeMap::default(), |g| {
        let test = g.provider().get_navigator("test");
        g.destination(NavDestinationBuilder::new_of::<Shell>(test.clone(), TypeMap::default()));
        g.destination(NavDestinationBuilder::new_of::<Start>(test.clone(), TypeMap::default()));
        g.destination(NavDestinationBuilder::new_of::<Detail>(test.clone(), TypeMap::default()));
        g.navigation_of::<AccountGraph>(Member(0), |account| {
            let mut m = NavDestinationBuilder::new_of::<Member>(test.clone(), TypeMap::default());
            m.deep_link_of::<Member>("kompoz://member");
            account.destination(m);
        });
        g.navigation("a/{n}", "grup", |grup| {
            let mut a = NavDestinationBuilder::new(test.clone(), Some("a/{n}".into()));
            a.argument("n", |b| b.set_type(NavType::IntType));
            a.deep_link_with(|l| l.set_uri_pattern(Some("kompoz://a/{n}".into())));
            grup.destination(a);
        });
    });
    nav.set_graph(graph);
    nav
}

fn stack_of(nav: &NavController) -> Vec<String> {
    nav.back_queue().iter().skip(1).map(|e| e.destination().route().unwrap_or_default()).collect()
}

/// The forms' text once composed again (the controller's `currentDestination` isn't state: Compose's
/// `currentBackStackEntryAsState`, kotlin_nav_host.rs, is).
fn text_of(nav: &NavController) -> KompozTestRule {
    let n = nav.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || NavForms(n.clone())));
    rule.wait_for_idle();
    rule
}

fn navigation_core_as_compose_writes_it() {
    let nav = controller();
    let rule = text_of(&nav);
    rule.on_node_with_text("hesapta false n 0 ayrıntı -1 kompoz://a/{n} kompoz://member/{n}").assert_exists();
    rule.on_node_with_text("hesap").perform_click();
    assert_eq!(stack_of(&nav), ["Shell", "AccountGraph", "Member/{n}"]);
    rule.on_node_with_text("kabuk").perform_click(); // the tab saved, the shell's again
    assert_eq!(stack_of(&nav), ["Shell"]);
    rule.on_node_with_text("hesap").perform_click(); // restored
    assert_eq!(stack_of(&nav), ["Shell", "AccountGraph", "Member/{n}"]);
    let rule = text_of(&nav);
    rule.on_node_with_text("hesapta true n 0 ayrıntı -1 kompoz://a/{n} kompoz://member/{n}").assert_exists();
    rule.on_node_with_text("geri").perform_click(); // popBackStack<AccountGraph>(inclusive = true)
    assert_eq!(stack_of(&nav), ["Shell"]);
    rule.on_node_with_text("ayrıntı").perform_click();
    assert_eq!(stack_of(&nav), ["Shell", "Detail/{id}?tab={tab}"]);
    let rule = text_of(&nav);
    rule.on_node_with_text("hesapta false n 0 ayrıntı 7 kompoz://a/{n} kompoz://member/{n}").assert_exists();
    rule.on_node_with_text("a").perform_click();
    assert_eq!(stack_of(&nav), ["Shell", "Detail/{id}?tab={tab}", "grup", "a/{n}"]);
    let rule = text_of(&nav);
    rule.on_node_with_text("hesapta false n 3 ayrıntı -1 kompoz://a/{n} kompoz://member/{n}").assert_exists();
    assert!(nav.handle_deep_link_uri("kompoz://member/5"));
    assert_eq!(nav.current_back_stack_entry().expect("one").to_route::<Member>().n, 5);
}

fn main() {
    common::run(&[("navigation_core_as_compose_writes_it", navigation_core_as_compose_writes_it)]);
}
