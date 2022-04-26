<template>
  <div>
    <h3>Project: {{ pid }} | Script: {{ id }}</h3>

    <form>
      <!-- Users input -->
      <div>
        <input
          type="text"
          id="users-input"
          class="form-control"
          v-model="users"
        />
        <label for="users-input">Users</label>
      </div>
      <!-- Spawn rate input -->
      <div>
        <input
          type="text"
          id="spawn-rate-input"
          class="form-control"
          v-model="spawnRate"
        />
        <label for="spawn-rate-input">Spawn rate</label>
      </div>
      <!-- Workers input -->
      <div>
        <input
          type="text"
          id="workers-input"
          class="form-control"
          v-model="workers"
        />
        <label for="workers-input">Workers</label>
      </div>
      <div class="form-text">This will overwrite all hosts in your file</div>
      <!-- Host input -->
      <div>
        <input
          type="text"
          id="host-input"
          class="form-control"
          v-model="host"
        />
        <label class="form-label" for="host-input">Host</label>
      </div>
      <div class="form-text">
        If time is not set, the test will not stop automatically
      </div>
      <!-- Time input -->
      <div>
        <input
          type="text"
          id="time-input"
          class="form-control"
          v-model="time"
        />
        <label class="form-label" for="time-input">Time in seconds</label>
      </div>
      <div class="form-text">Descripe your test</div>
      <!-- Label input -->
      <div>
        <input
          type="text"
          id="description-input"
          class="form-control"
          v-model="description"
        />
        <label for="description-input">Description</label>
      </div>
      <!-- Submit button -->
      <button type="button" id="start-btn" @click="start">Start</button>
    </form>
  </div>
</template>

<script>
export default {
  name: "Script",
  props: ["pid", "id"],
  data() {
    return {
      users: null,
      spawnRate: null,
      workers: null,
      host: null,
      time: null,
      description: null,
    };
  },
  methods: {
    start() {
      console.log(
        this.users,
        this.spawnRate,
        this.workers,
        this.host,
        this.time,
        this.description
      );
      fetch("/api/worker/start_test", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          project_id: this.pid,
          script_id: this.id,
          users: parseInt(this.users),
          spawn_rate:  parseInt(this.spawnRate),
          workers:  parseInt(this.workers),
          host: this.host,
          time:  parseInt(this.time),
          description: this.description,
        }),
      })
        .then((data) => data.json())
        .then((data) => {})
        .catch(() => {});
    },
  },
};
</script>

<style>
</style>