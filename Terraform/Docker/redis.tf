resource "docker_image" "redis" {
  name         = "redis:latest"
  keep_locally = false
}

resource "docker_container" "redis" {
  image = docker_image.redis.image_id
  name  = "redis"
  networks_advanced {
    name = docker_network.network.name
  }
  restart = local.container_restart_policy
}