#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Bars(shown: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(|| {
        TopAppBar(
            title = || {
                ColumnAnimatedVisibility(shown, |this| {
                    let _ = &this;
                    Text("başlık");
                });
            },
            actions = || {
                RowAnimatedVisibility(shown, |this| {
                    let _ = &this;
                    Text("eylem");
                });
            },
        );
        TopAppBarWithSubtitle(
            title = || {
                Text("başlık");
            },
            subtitle = || {
                Text("alt");
            },
            actions = || {
                RowAnimatedVisibility(shown, |this| {
                    let _ = &this;
                    Text("alt eylem");
                });
            },
        );
        CenterAlignedTopAppBar(
            title = || {
                Text("orta");
            },
            actions = || {
                RowAnimatedVisibility(shown, |this| {
                    let _ = &this;
                    Text("orta eylem");
                });
            },
        );
        MediumTopAppBar(
            title = || {
                Text("orta boy");
            },
            actions = || {
                RowAnimatedVisibility(shown, |this| {
                    let _ = &this;
                    Text("orta boy eylem");
                });
            },
        );
        LargeFlexibleTopAppBar(
            title = || {
                Text("büyük");
            },
            actions = || {
                RowAnimatedVisibility(shown, |this| {
                    let _ = &this;
                    Text("büyük eylem");
                });
            },
        );
        TwoRowsTopAppBar(
            title = |expanded| {
                let _ = &expanded;
                Text("iki");
            },
            actions = || {
                RowAnimatedVisibility(shown, |this| {
                    let _ = &this;
                    Text("iki eylem");
                });
            },
        );
    });
}
