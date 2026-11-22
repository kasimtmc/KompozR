// Kotlin's imports of a class's or object's members (material3's FloatingToolbarSamples: `import
// …FloatingToolbarDefaults.ScreenOffset`, `import …FloatingToolbarExitDirection.Companion.Bottom`): the bare name is
// the member, qualified (a companion's members are its class's); an alias names it otherwise; a nested object
// imported is the qualified one (`Absolute.Left` → `Arrangement.Absolute.Left`); a member function called bare is
// its object's; a local of the name is nearer; imports of classes, top-level functions and packages (`.*`) are left
// to the Rust module's own `use`s.
import androidx.compose.foundation.layout.Arrangement.Absolute
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.*
import androidx.compose.material3.FloatingToolbarDefaults.ScreenOffset
import androidx.compose.material3.FloatingToolbarDefaults.exitAlwaysScrollBehavior
import androidx.compose.material3.FloatingToolbarExitDirection.Companion.Bottom
import androidx.compose.material3.FloatingToolbarExitDirection.Companion.End as Away
import androidx.compose.ui.unit.dp

@Composable
fun Imported(expanded: Boolean) {
    val below = exitAlwaysScrollBehavior(exitDirection = Bottom)
    val beside = FloatingToolbarDefaults.exitAlwaysScrollBehavior(exitDirection = Away)
    Row(horizontalArrangement = Absolute.Left) {
        HorizontalFloatingToolbar(expanded, Modifier.offset(y = -ScreenOffset), scrollBehavior = below) { Text("a") }
        VerticalFloatingToolbar(expanded, Modifier.offset(x = -ScreenOffset), scrollBehavior = beside) { Text("b") }
    }
}

@Composable
fun Shadowed() {
    val ScreenOffset = 4.dp
    Box(Modifier.padding(ScreenOffset))
}
