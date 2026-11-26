// (stage 14 JUMP) Jumps out of the lambdas of composable calls, or of lambdas calling composables: a `return` out of
// `Column`'s content, `break`/`continue` of a loop out of `Row`'s, `return@Column` out of a `Row` inside it, out of
// `key`'s and the file's own inline composable's: `kt::leave` in the lambda, `kt::leaving` around the call (its slot a
// reference), which ends the groups the jump leaves (the composer's `end_to_marker`, Compose's `endToMarker`).

@Composable
fun Rows(xs: List<String>) {
    Column {
        if (xs.isEmpty()) return
        Text(xs.first())
    }
}

@Composable
fun Cut(xs: List<String>, limit: Int) {
    Column {
        for ((i, x) in xs.withIndex()) {
            Row {
                if (i >= limit) break
                if (x == "iki") continue
                Text("satır $x")
            }
        }
        Text("kesildi")
    }
}

@Composable
fun Labeled(xs: List<String>) {
    Column {
        Row {
            if (xs.size > 1) return@Column
            Text("tek")
        }
        Text("çok değil")
    }
    Text("etiket sonu")
}

@Composable
inline fun Framed(content: @Composable () -> Unit) {
    Box { content() }
}

@Composable
fun Keyed(xs: List<String>): Int {
    for (x in xs) {
        key(x) {
            Framed {
                if (x.isEmpty()) return -1
                Text(x)
            }
        }
    }
    return xs.size
}
