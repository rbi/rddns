workflow "build-workflow" {
  on = "push"
  resolves = ["push docker images"]
}

action "build x86_64 linux" {
  uses = "actions/docker/cli@8cdf801b322af5f369e00d85e9cf3a7122f49108"
  args = "build -t sirabien/rddns:dev ."
}

action "when on master" {
  uses = "actions/bin/filter@3c98a2679187369a2116d4f311568596d3725740"
  needs = ["build x86_64 linux"]
  args = "branch master"
}

action "login to Docker Hub" {
  uses = "actions/docker/login@8cdf801b322af5f369e00d85e9cf3a7122f49108"
  needs = ["when on master"]
  secrets = ["DOCKER_USERNAME", "DOCKER_PASSWORD"]
}

action "push docker images" {
  uses = "actions/docker/cli@8cdf801b322af5f369e00d85e9cf3a7122f49108"
  args = "push sirabien/rddns:dev"
  needs = ["login to Docker Hub"]
}
