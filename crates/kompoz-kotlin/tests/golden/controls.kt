// Material's selection controls: `null` for a nullable callback (`onCheckedChange = null`), a state's setter of a
// destructuring given as one (`Rc::new` of it), kotlin.math's `floor`, `toPx()` in `with(density)`, the overloads of
// `XDefaults.colors(…)` by their parameters' names, `toggleable(…, indication = null, …)`.

@Composable
fun Controls() {
    val (checked, onCheckedChange) = remember { mutableStateOf(true) }
    val strokeWidthPx = with(LocalDensity.current) { floor(CheckboxDefaults.StrokeWidth.toPx()) }
    Row(Modifier.toggleable(value = checked, onValueChange = onCheckedChange, role = Role.Checkbox, indication = null, interactionSource = null)) {
        Checkbox(checked, null)
        Checkbox(checked = checked, onCheckedChange = onCheckedChange, colors = CheckboxDefaults.colors(checkedColor = Color.Blue))
        TriStateCheckbox(state = ToggleableState.On, onClick = null, colors = CheckboxDefaults.colors(checkedBoxColor = Color.Blue))
        Checkbox(checked, onCheckedChange, Stroke(width = strokeWidthPx), Stroke(width = strokeWidthPx), colors = CheckboxDefaults.colors())
        RadioButton(selected = checked, onClick = null)
        Switch(checked = checked, onCheckedChange = { onCheckedChange(it) }, thumbContent = null)
    }
}
