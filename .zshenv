# For MacOS
# Copy the following content to your ~/.zshenv
# you might have to logout and login if GUI apps don't get the updated PATH
path_additions=(
  /opt/homebrew/opt/cups/bin
  /Applications/LibreOffice.app/Contents/MacOS
)

IFS=:
export PATH="${path_additions[*]}:$PATH"
unset IFS

export DYLD_LIBRARY_PATH="/opt/homebrew/lib:$DYLD_LIBRARY_PATH"
export MAGICK_HOME="/opt/homebrew/opt/imagemagick"
export PYTHONUNBUFFERED=1
export TOKENIZERS_PARALLELISM=false
export DOCKER_BUILDKIT=1