import { ref, onMounted, onUnmounted, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open, confirm } from '@tauri-apps/plugin-dialog'
import type { SoundBinding, SoundSets, SoundSet } from '../types'
import { debugLog, debugError } from '../utils/debug'

export function useSoundPanel() {
  const bindings = ref<SoundBinding[]>([])
  const errorMessage = ref<string | null>(null)
  const showAddDialog = ref(false)
  const showAddSetDialog = ref(false)
  const newSetName = ref('')
  const addSetInputRef = ref<HTMLInputElement | null>(null)
  const isLoading = ref(false)

  const newKey = ref('A')
  const newDescription = ref('')
  const newFilePath = ref('')
  const isTesting = ref(false)
  const isSaving = ref(false)

  const _cleanups: Array<() => void> = []
  onUnmounted(() => {
    _cleanups.forEach(fn => fn())
    _cleanups.length = 0
  })

  const availableKeys = Array.from({ length: 26 }, (_, i) =>
    String.fromCharCode(65 + i)
  )

  const sets = ref<SoundSet[]>([])
  const activeSetId = ref<string>('')
  const editingSetId = ref<string | null>(null)
  const editingSetName = ref('')
  const editingInputRef = ref<HTMLInputElement | null>(null)

  async function loadSets() {
    try {
      const result = await invoke<SoundSets>('sp_get_sets')
      sets.value = result.sets || []
      activeSetId.value = result.active_set_id || ''
      debugLog('[SoundPanelTab] Loaded sets:', sets.value.length, 'active:', activeSetId.value)
    } catch (e) {
      debugError('[SoundPanelTab] Failed to load sets:', e)
    }
  }

  async function switchSet(id: string) {
    if (id === activeSetId.value) return
    try {
      await invoke('sp_set_active_set', { id })
      activeSetId.value = id
      await loadBindings()
    } catch (e) {
      showError('Ошибка переключения набора: ' + (e as Error).message)
    }
  }

  function addSet() {
    newSetName.value = ''
    showAddSetDialog.value = true
    nextTick(() => {
      addSetInputRef.value?.focus()
    })
  }

  async function confirmAddSet() {
    const name = newSetName.value.trim()
    if (!name) return
    try {
      const created = await invoke<SoundSet>('sp_add_set', { name })
      await loadSets()
      activeSetId.value = created.id
      bindings.value = []
      showAddSetDialog.value = false
    } catch (e) {
      showError('Ошибка создания набора: ' + (e as Error).message)
    }
  }

  function closeAddSetDialog() {
    showAddSetDialog.value = false
    newSetName.value = ''
  }

  function startRename(set: SoundSet) {
    editingSetId.value = set.id
    editingSetName.value = set.name
    nextTick(() => {
      editingInputRef.value?.focus()
      editingInputRef.value?.select()
    })
  }

  async function saveRename(id: string) {
    const name = editingSetName.value.trim()
    editingSetId.value = null
    if (!name || !name.trim()) return
    try {
      await invoke('sp_rename_set', { id, name })
      await loadSets()
    } catch (e) {
      showError('Ошибка переименования: ' + (e as Error).message)
    }
  }

  function cancelRename() {
    editingSetId.value = null
  }

  function onRenameKeydown(e: KeyboardEvent, id: string) {
    if (e.key === 'Enter') saveRename(id)
    if (e.key === 'Escape') cancelRename()
  }

  async function removeSet(id: string) {
    const set = sets.value.find(s => s.id === id)
    const name = set ? `"${set.name}"` : id
    const confirmedResult = await confirm(`Удалить набор ${name}? Аудиофайлы останутся.`, {
      title: 'Удалить набор',
      kind: 'warning'
    })
    if (!confirmedResult) return
    try {
      await invoke('sp_remove_set', { id })
      await loadSets()
      await loadBindings()
    } catch (e) {
      showError('Ошибка удаления набора: ' + (e as Error).message)
    }
  }

  async function loadBindings() {
    try {
      isLoading.value = true
      const loaded = await invoke<SoundBinding[]>('sp_get_bindings')
      bindings.value = loaded
    } catch (e) {
      showError('Ошибка загрузки привязок: ' + (e as Error).message)
    } finally {
      isLoading.value = false
    }
  }

  async function addBinding() {
    if (!newKey.value || !newDescription.value || !newFilePath.value) {
      showError('Заполните все поля')
      return
    }

    try {
      isSaving.value = true
      const binding = await invoke<SoundBinding>('sp_add_binding', {
        key: newKey.value,
        description: newDescription.value,
        filePath: newFilePath.value
      })
      bindings.value.push(binding)
      bindings.value.sort((a, b) => a.key.localeCompare(b.key))
      closeAddDialog()
    } catch (e) {
      showError('Ошибка добавления: ' + (e as Error).message)
    } finally {
      isSaving.value = false
    }
  }

  async function removeBinding(key: string) {
    const confirmedResult = await confirm(`Удалить привязку для клавиши ${key}?`, {
      title: 'Подтверждение удаления',
      kind: 'warning'
    })
    if (!confirmedResult) return
    try {
      await invoke('sp_remove_binding', { key })
      bindings.value = bindings.value.filter(b => b.key !== key)
    } catch (e) {
      showError('Ошибка удаления: ' + (e as Error).message)
    }
  }

  async function testSound() {
    if (!newFilePath.value) {
      showError('Выберите файл')
      return
    }
    try {
      isTesting.value = true
      await invoke('sp_test_sound', { filePath: newFilePath.value })
    } catch (e) {
      showError('Ошибка воспроизведения: ' + (e as Error).message)
    } finally {
      isTesting.value = false
    }
  }

  async function browseFile() {
    try {
      debugLog('[browseFile] Opening file dialog...')
      const filePath = await open({
        title: 'Выберите аудиофайл',
        multiple: false,
        filters: [
          {
            name: 'Аудиофайлы',
            extensions: ['mp3', 'wav', 'ogg', 'flac']
          }
        ]
      })
      if (filePath) {
        const pathStr = typeof filePath === 'string' ? filePath : String(filePath)
        newFilePath.value = pathStr
      }
    } catch (e) {
      debugError('[browseFile] Error:', e)
      showError('Ошибка выбора файла: ' + (e as Error).message)
    }
  }

  function closeAddDialog() {
    showAddDialog.value = false
    newKey.value = 'A'
    newDescription.value = ''
    newFilePath.value = ''
  }

  function showError(message: string) {
    errorMessage.value = message
    setTimeout(() => errorMessage.value = null, 5000)
  }

  function getAvailableKeys(): string[] {
    const usedKeys = new Set(bindings.value.map(b => b.key))
    return availableKeys.filter(key => !usedKeys.has(key))
  }

  onMounted(async () => {
    await loadSets()
    await loadBindings()

    const unlistenBindings = await listen('soundpanel-bindings-changed', async () => {
      debugLog('[SoundPanelTab] Bindings changed event, reloading')
      await loadSets()
      await loadBindings()
    })
    _cleanups.push(() => unlistenBindings())

    const unlistenActiveSet = await listen('soundpanel-active-set-changed', async () => {
      debugLog('[SoundPanelTab] Active set changed event, reloading')
      await loadSets()
      await loadBindings()
    })
    _cleanups.push(() => unlistenActiveSet())
  })

  return {
    bindings,
    errorMessage,
    showAddDialog,
    showAddSetDialog,
    newSetName,
    addSetInputRef,
    isLoading,
    newKey,
    newDescription,
    newFilePath,
    isTesting,
    isSaving,
    availableKeys,
    sets,
    activeSetId,
    editingSetId,
    editingSetName,
    editingInputRef,
    loadSets,
    switchSet,
    addSet,
    confirmAddSet,
    closeAddSetDialog,
    startRename,
    saveRename,
    cancelRename,
    onRenameKeydown,
    removeSet,
    loadBindings,
    addBinding,
    removeBinding,
    testSound,
    browseFile,
    closeAddDialog,
    showError,
    getAvailableKeys,
  }
}
