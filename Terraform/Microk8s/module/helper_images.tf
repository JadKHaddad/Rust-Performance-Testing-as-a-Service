resource "docker_image" "builder" {
  name         = "builder:latest"
  keep_locally = true
  build {
    path       = local.paths.image.context_path
    dockerfile = "${local.paths.image.dockerfiles_path}/Dockerfile.builder"
  }
}

resource "docker_image" "runner" {
  name         = "runner:latest"
  keep_locally = true
  build {
    path       = local.paths.image.context_path
    dockerfile = "${local.paths.image.dockerfiles_path}/Dockerfile.runner"
  }
}