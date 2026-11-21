// Material's menus as Compose writes them: ExposedDropdownMenuBox's content a lambda of its scope — `Modifier.menuAnchor(…)`
// its member extension, `ExposedDropdownMenu(…)` its composable member (a PascalCase call of the implicit receiver
// before any function of that name), `scrollState = rememberScrollState()` to its optional parameter —; DropdownMenuItem's
// overloads by their parameters (`checked`, `selected`, `shape`/`shapes`), its text and onClick positional (the
// macro takes a last unnamed closure for the last parameter: named); MenuDefaults' properties (`shape`,
// `TrailingIconSize`) and functions; a slot given `x?.let { name -> { … } }`; a list of nullable icons (`listOf(null,
// x)`, nested, declared); `Int == Int?`; a delegated `Int?` set from an `Int`; the deprecated `MenuAnchorType` alias.
@Composable
fun Picker(options: List<String>, supporting: List<String?>) {
    var expanded by remember { mutableStateOf(false) }
    var picked: Int? by remember { mutableStateOf(null) }
    val icons = listOf(listOf(Icons.Filled.Edit, Icons.Filled.Home), listOf(null, Icons.Filled.Info))
    val declared: List<ImageVector?> = listOf(null, null)
    ExposedDropdownMenuBox(expanded = expanded, onExpandedChange = { expanded = it }) {
        TextField(
            modifier = Modifier.menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable).fillMaxWidth(),
            value = options[0],
            onValueChange = {},
            readOnly = true,
            label = { Text("Seç") },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
            colors = ExposedDropdownMenuDefaults.textFieldColors(),
        )
        ExposedDropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }, scrollState = rememberScrollState(), shape = MenuDefaults.shape) {
            options.forEachIndexed { index, option ->
                DropdownMenuItem(
                    text = { Text(option) },
                    selected = index == picked,
                    onClick = { picked = index },
                    shapes = MenuDefaults.itemShape(index, options.size),
                    supportingText = supporting[index]?.let { s -> { Text(s) } },
                    leadingIcon = icons[0][index]?.let { { Icon(it, contentDescription = null, modifier = Modifier.size(MenuDefaults.TrailingIconSize)) } },
                    contentPadding = ExposedDropdownMenuDefaults.ItemContentPadding,
                )
            }
            DropdownMenuItem(checked = expanded, onCheckedChange = { expanded = it }, text = { Text("İşaretli") }, shapes = MenuDefaults.itemShape(0, 1))
            DropdownMenuItem(onClick = {}, text = { Text("Biçimli") }, shape = MenuDefaults.standaloneItemShape)
            DropdownMenuItem({ Text("Konumla") }, { expanded = false })
        }
    }
}

@Composable
fun Old() {
    ExposedDropdownMenuBox(expanded = false, onExpandedChange = {}) {
        TextField(state = rememberTextFieldState(), modifier = Modifier.menuAnchor(MenuAnchorType.PrimaryEditable))
    }
}
