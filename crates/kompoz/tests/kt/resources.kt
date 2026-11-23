// Resources as Compose Multiplatform's users write them (`Res.string.x`, `Res.drawable.x`, `Res.plurals.x`,
// `Res.array.x`, `Res.font.x`; stringResource with format arguments, pluralStringResource, stringArrayResource,
// painterResource of a bitmap and of a vector drawable, imageResource, vectorResource, Font of a font resource), and
// as Android's write them (`R.string.x`, `painterResource(id = R.drawable.x)`, `pluralStringResource(id, count)`):
// both tests/composeResources' `Res`.

@Composable
fun ResourcesScreen(count: Int, name: String) {
    Column {
        Text(stringResource(Res.string.title), modifier = Modifier.testTag("title"))
        Text(stringResource(Res.string.welcome, name), modifier = Modifier.testTag("welcome"))
        Text(pluralStringResource(Res.plurals.items, count, count), modifier = Modifier.testTag("items"))
        Text(stringArrayResource(Res.array.days).joinToString(", "), modifier = Modifier.testTag("days"))
        Image(painterResource(Res.drawable.badge), contentDescription = "badge")
        Image(painter = painterResource(Res.drawable.arrow), contentDescription = "arrow")
        Image(bitmap = imageResource(Res.drawable.badge), contentDescription = "bitmap")
        Image(imageVector = vectorResource(Res.drawable.arrow), contentDescription = "vector")
        Text("kalın", fontFamily = FontFamily(Font(Res.font.bold, FontWeight.Bold)), modifier = Modifier.testTag("font"))
    }
}

@Composable
fun AndroidResources(count: Int) {
    Column {
        Text(stringResource(id = R.string.title), modifier = Modifier.testTag("android title"))
        Text(pluralStringResource(id = R.plurals.items, count = count, count), modifier = Modifier.testTag("android items"))
        Image(painter = painterResource(id = R.drawable.badge), contentDescription = stringResource(R.string.title))
    }
}
