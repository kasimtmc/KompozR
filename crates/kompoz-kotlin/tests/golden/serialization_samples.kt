// (stage 14 FINAL) lifecycle-viewmodel-compose 2.11's SnapshotStateViewModel samples (a state list and a state map
// saved by the handle through `listSaver`, the declared `MutableList`/`MutableMap` the state collections they are,
// `listSaver`'s `T` bound by the type the call is to have, `by handle.saveable(…)` a delegate of the value or of the
// state) and savedstate 1.4's codec samples' `class UUIDSerializer : KSerializer<UUID>` (kotlinx's serializer of one's
// own: `KSerializerImpl`, given where a serializer is).

@Sampled
fun SnapshotStateViewModel() {

    /** A simple item that is not inherently [Parcelable] */
    data class Item(val id: UUID, val value: String)

    @OptIn(SavedStateHandleSaveableApi::class)
    class SnapshotStateViewModel(handle: SavedStateHandle) : ViewModel() {

        /**
         * A snapshot-backed [MutableList] of a list of items, persisted by the [SavedStateHandle].
         * The size of this set must remain small in expectation, since the maximum size of saved
         * instance state space is limited.
         */
        private val items: MutableList<Item> =
            handle.saveable(
                key = "items",
                saver =
                    listSaver(
                        save = { it.map { item -> listOf(item.id.toString(), item.value) } },
                        restore = {
                            it.map { saved ->
                                    Item(id = UUID.fromString(saved[0]), value = saved[1])
                                }
                                .toMutableStateList()
                        },
                    ),
            ) {
                mutableStateListOf()
            }

        /**
         * A snapshot-backed [MutableMap] representing a set of selected item ids, persisted by the
         * [SavedStateHandle]. A [MutableSet] is approximated by ignoring the keys. The size of this
         * set must remain small in expectation, since the maximum size of saved instance state
         * space is limited.
         */
        private val selectedItemIds: MutableMap<UUID, Unit> =
            handle.saveable(
                key = "selectedItemIds",
                saver =
                    listSaver(
                        save = { it.keys.map(UUID::toString) },
                        restore = {
                            it.map(UUID::fromString).map { id -> id to Unit }.toMutableStateMap()
                        },
                    ),
            ) {
                mutableStateMapOf()
            }

        /**
         * A snapshot-backed flag representing where selections are enabled, persisted by the
         * [SavedStateHandle].
         */
        var areSelectionsEnabled by handle.saveable("areSelectionsEnabled") { mutableStateOf(true) }

        /** A list of items paired with a selection state. */
        val selectedItems: List<Pair<Item, Boolean>>
            get() = items.map { it to (it.id in selectedItemIds) }

        /** Updates the selection state for the item with [id] to [selected]. */
        fun selectItem(id: UUID, selected: Boolean) {
            if (selected) {
                selectedItemIds[id] = Unit
            } else {
                selectedItemIds.remove(id)
            }
        }

        /** Adds an item with the given [value]. */
        fun addItem(value: String) {
            items.add(Item(UUID.randomUUID(), value))
        }
    }
}

@Sampled
fun SnapshotStateViewModelWithDelegates() {

    /** A simple item that is not inherently [Parcelable] */
    data class Item(val id: UUID, val value: String)

    @OptIn(SavedStateHandleSaveableApi::class)
    class SnapshotStateViewModel(handle: SavedStateHandle) : ViewModel() {

        /**
         * A snapshot-backed [MutableList] of a list of items, persisted by the [SavedStateHandle].
         * The size of this set must remain small in expectation, since the maximum size of saved
         * instance state space is limited.
         */
        private val items: MutableList<Item> by
            handle.saveable(
                saver =
                    listSaver(
                        save = { it.map { item -> listOf(item.id.toString(), item.value) } },
                        restore = {
                            it.map { saved ->
                                    Item(id = UUID.fromString(saved[0]), value = saved[1])
                                }
                                .toMutableStateList()
                        },
                    )
            ) {
                mutableStateListOf()
            }

        /**
         * A snapshot-backed [MutableMap] representing a set of selected item ids, persisted by the
         * [SavedStateHandle]. A [MutableSet] is approximated by ignoring the keys. The size of this
         * set must remain small in expectation, since the maximum size of saved instance state
         * space is limited.
         */
        private val selectedItemIds: MutableMap<UUID, Unit> by
            handle.saveable(
                saver =
                    listSaver(
                        save = { it.keys.map(UUID::toString) },
                        restore = {
                            it.map(UUID::fromString).map { id -> id to Unit }.toMutableStateMap()
                        },
                    )
            ) {
                mutableStateMapOf()
            }

        /**
         * A snapshot-backed flag representing where selections are enabled, persisted by the
         * [SavedStateHandle].
         */
        var areSelectionsEnabled by handle.saveable { mutableStateOf(true) }

        /** A list of items paired with a selection state. */
        val selectedItems: List<Pair<Item, Boolean>>
            get() = items.map { it to (it.id in selectedItemIds) }

        /** Updates the selection state for the item with [id] to [selected]. */
        fun selectItem(id: UUID, selected: Boolean) {
            if (selected) {
                selectedItemIds[id] = Unit
            } else {
                selectedItemIds.remove(id)
            }
        }

        /** Adds an item with the given [value]. */
        fun addItem(value: String) {
            items.add(Item(UUID.randomUUID(), value))
        }
    }
}


@Sampled
fun encodeWithExplicitSerializer() {
    class UUIDSerializer : KSerializer<UUID> {
        override val descriptor: SerialDescriptor
            get() = PrimitiveSerialDescriptor("UUIDSerializer", PrimitiveKind.STRING)

        override fun deserialize(decoder: Decoder): UUID {
            return UUID.fromString(decoder.decodeString())
        }

        override fun serialize(encoder: Encoder, value: UUID) {
            encoder.encodeString(value.toString())
        }
    }
    encodeToSavedState(UUIDSerializer(), UUID.randomUUID())
}
@Sampled
fun decodeWithExplicitSerializer() {
    class UUIDSerializer : KSerializer<UUID> {
        override val descriptor: SerialDescriptor
            get() = PrimitiveSerialDescriptor("UUIDSerializer", PrimitiveKind.STRING)

        override fun deserialize(decoder: Decoder): UUID {
            return UUID.fromString(decoder.decodeString())
        }

        override fun serialize(encoder: Encoder, value: UUID) {
            encoder.encodeString(value.toString())
        }
    }
    val uuid = decodeFromSavedState(UUIDSerializer(), uuidSavedState)
}
val uuidSavedState = savedState { putString("", UUID.randomUUID().toString()) }
