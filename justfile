# use just.systems variable for the ollama version
# https://github.com/ollama/ollama/releases 
ollama_version := "v0.5.7"

# This is called from github/workflows, if you change this name, change that file too
install_ollama_linux:
  echo "Installing ollama on Linux"
  # wget https://github.com/jmorganca/ollama/releases/download/v0.1.20/ollama-darwin 
  wget https://github.com/ollama/ollama/releases/download/{{ollama_version}}/ollama-linux-amd64.tgz
  # Now that we've downloaded the tarball, we need to extract it 
  mkdir -p ollama_files
  tar -xvzf ollama-linux-amd64.tgz --directory ollama_files
  # Make the binary executable 
  chmod +x ollama_files/bin/ollama
  # remove the tar 
  rm ollama-linux-amd64.tgz
