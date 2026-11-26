// (stage 14 FINAL) A generic composable's type parameter bounded as the API's generic function it is given to bounds
// it (adaptive-layout 1.2.0's reflowAdaptStrategySample<T> and levitateAsDialogSample<T>: `T: NavigatorContentKey`,
// `rememberListDetailPaneScaffoldNavigator<T>`'s, the index's FN_BOUNDS); a nullable local `var` returned after a
// non-null value was assigned to it (Kotlin's smart cast).

@OptIn(ExperimentalMaterial3AdaptiveApi::class)
@Preview
@Sampled
@Composable
fun <T> reflowAdaptStrategySample(): ThreePaneScaffoldNavigator<T> =
    rememberListDetailPaneScaffoldNavigator<T>(
        adaptStrategies =
            ListDetailPaneScaffoldDefaults.adaptStrategies(
                extraPaneAdaptStrategy =
                    AdaptStrategy.Reflow(reflowUnder = ListDetailPaneScaffoldRole.Detail)
            )
    )

/**
 * This sample shows how to create a [ThreePaneScaffoldNavigator] that will show the extra pane as a
 * modal dialog when the extra pane is the current destination. The dialog will be centered in the
 * scaffold, with a scrim that clicking on it will dismiss the dialog.
 */
@OptIn(ExperimentalMaterial3AdaptiveApi::class)
@Preview
@Sampled
@Composable
fun <T> levitateAsDialogSample(): ThreePaneScaffoldNavigator<T> {
    val coroutineScope = rememberCoroutineScope()
    val scaffoldDirective = calculatePaneScaffoldDirective(currentWindowAdaptiveInfo())
    var navigator: ThreePaneScaffoldNavigator<T>? = null
    val onClick: () -> Unit = { coroutineScope.launch { navigator?.navigateBack() } }
    navigator =
        rememberListDetailPaneScaffoldNavigator<T>(
            scaffoldDirective = scaffoldDirective,
            adaptStrategies =
                SupportingPaneScaffoldDefaults.adaptStrategies(
                    extraPaneAdaptStrategy =
                        AdaptStrategy.Levitate(
                                alignment = Alignment.Center,
                                scrim = {
                                    LevitatedPaneScrim(
                                        Modifier.semantics {
                                            contentDescription = "Scrim"
                                            this.onClick("Dismiss the extra pane") {
                                                onClick()
                                                true
                                            }
                                        },
                                        onClick = onClick,
                                    )
                                },
                            )
                            .onlyIfSinglePane(scaffoldDirective)
                ),
        )
    return navigator
}
