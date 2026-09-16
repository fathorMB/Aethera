# Percorsi della macchina per gli script dei banchi (m08, m10, m11).
# I valori veri stanno in .lmbrain-lite/percorsi.local.sh, che non si committa (vedi .gitignore);
# percorsi.esempio.sh mostra la forma. Si usano da bash:  . "$(dirname "$0")/../percorsi.sh"
_percorsi_qui=$(dirname "${BASH_SOURCE[0]}")
[ -f "$_percorsi_qui/percorsi.local.sh" ] && . "$_percorsi_qui/percorsi.local.sh"
: "${AETHERA_RADICE:?imposta AETHERA_RADICE (radice dati di Aethera) in .lmbrain-lite/percorsi.local.sh}"
: "${AETHERA_REPO:?imposta AETHERA_REPO (checkout di Aethera) in .lmbrain-lite/percorsi.local.sh}"
export AETHERA_RADICE AETHERA_REPO AETHERA_PESI AETHERA_BUILD AETHERA_DOWNLOAD
