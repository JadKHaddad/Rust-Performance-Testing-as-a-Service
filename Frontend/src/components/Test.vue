<template>
  <div class="test-container">
    <div class="uk-overflow-auto">
      <div  class="uk-grid-small uk-child-width-1-4@s uk-text-center" uk-grid>
        <div class="uk-width-auto@m">
          <button class="uk-button uk-button-danger" type="button" @click="stop_me()">Stop</button>
        </div>
        <div class="uk-width-auto@m">
        <button class="uk-button uk-button-danger" type="button" @click="delete_me()">Delete</button>
      </div>
        <div class="uk-width-auto@m">
        <button class="uk-button uk-button-primary" type="button" @click="restart_me()">Restart</button>
      </div>
        <div class="uk-width-auto@m">
        <button class="uk-button uk-button-primary" type="button" @click="download_me()">Download</button>
      </div>
      </div>
      <table
        class="uk-table uk-table-small uk-table-striped uk-table-responsive"
      >
        <thead>
          <tr>
            <th class="text">{{ test.id }}</th>
            <th class="text">{{ test.info.users }}</th>
            <th class="text">{{ test.info.spawn_rate }}</th>
            <th class="text">{{ test.info.workers }}</th>
            <th class="text">{{ test.info.host }}</th>
            <th class="text">{{ test.info.time }}</th>
            <th></th>
            <th></th>
            <th></th>
            <th></th>
            <th><div v-if="test.status == 0" uk-spinner></div></th>
          </tr>
        </thead>
        <thead>
          <tr>
            <th class="text">Type</th>
            <th class="text">Name</th>
            <th class="text">Requests</th>
            <th class="text">Failures</th>
            <th class="text">Med Res Time</th>
            <th class="text">Avg Res Time</th>
            <th class="text">Min Res Time</th>
            <th class="text">Max Res Time</th>
            <th class="text">Avg Content Size</th>
            <th class="text">Requests/s</th>
            <th class="text">Failures/s</th>
          </tr>
        </thead>
        <tbody>
          <tr class="row" v-for="result in test.results" :key="result">
            <td class="text">{{ result.type }}</td>
            <td class="text">{{ result.name }}</td>
            <td class="text">{{ result.request_count }}</td>
            <td class="text">{{ result.failure_count }}</td>
            <td class="text">{{ result.median_response_time.slice(0, 6) }}</td>
            <td class="text">{{ result.avarage_response_time.slice(0, 6) }}</td>
            <td class="text">{{ result.min_response_time.slice(0, 6) }}</td>
            <td class="text">{{ result.max_response_time.slice(0, 6) }}</td>
            <td class="text">{{ result.avarage_content_size.slice(0, 6) }}</td>
            <td class="text">{{ result.requests_per_second.slice(0, 6) }}</td>
            <td class="text">{{ result.failures_per_seconde.slice(0, 6) }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <br />
    <br />
    <br />
    <div
      :id="test.id + '-chartContainer'"
      style="height: 370px; width: 100%"
    ></div>
    <br />
    <br />
    <br />
  </div>
</template>

<script>
export default {
  name: "Test",
  props: ["test", "darkTheme"],
  watch: {
    darkTheme: function (newVal, oldVal) {
      this.chart.options.theme = newVal ? "dark1" : "light1";
      this.chart.render();
    },
    test: {
      handler(newVal) {
        const lastHistory = newVal.last_history;
        if (lastHistory != null) {
          const date = new Date(parseInt(lastHistory.timestamp) * 1000);
          this.total_median_response_time.dataPoints.push({
            x: date,
            y: parseInt(lastHistory.total_median_response_time),
          });
          this.total_average_response_time.dataPoints.push({
            x: date,
            y: parseInt(lastHistory.total_average_response_time),
          });
          this.total_min_response_time.dataPoints.push({
            x: date,
            y: parseInt(lastHistory.total_min_response_time),
          });
          this.total_max_response_time.dataPoints.push({
            x: date,
            y: parseInt(lastHistory.total_max_response_time),
          });
          this.chart.render();
        }
      },
      deep: true,
    },
  },
  data() {
    return {
      chart: null,
      total_median_response_time: {
        name: "Total Median Response Time",
        type: "line",
        showInLegend: true,
        dataPoints: [],
      },
      total_average_response_time: {
        name: "Total Average Response Time",
        type: "line",
        showInLegend: true,
        dataPoints: [],
      },
      total_min_response_time: {
        name: "Total Min Response Time",
        type: "line",
        showInLegend: true,
        dataPoints: [],
      },
      total_max_response_time: {
        name: "Total Max Response Time",
        type: "line",
        showInLegend: true,
        dataPoints: [],
      },
    };
  },

  methods: {
    stop_me() {
      this.$emit("stop_me");
    },
    delete_me() {
      this.$emit("delete_me");
    },
    restart_me() {
      this.$emit("restart_me");
    },
    download_me() {
      this.$emit("download_me");
    },
    toggleDataSeries(e) {
      if (typeof e.dataSeries.visible === "undefined" || e.dataSeries.visible) {
        e.dataSeries.visible = false;
      } else {
        e.dataSeries.visible = true;
      }
      this.chart.render();
    },
    setupChart() {
      if (this.test.history != null && this.test.history.length > 0) {
        for (var i = 0; i < this.test.history.length; i++) {
          const record = this.test.history[i];
          const date = new Date(parseInt(record.timestamp) * 1000);
          this.total_median_response_time.dataPoints.push({
            x: date,
            y: parseInt(record.total_median_response_time),
          });
          this.total_average_response_time.dataPoints.push({
            x: date,
            y: parseInt(record.total_average_response_time),
          });
          this.total_min_response_time.dataPoints.push({
            x: date,
            y: parseInt(record.total_min_response_time),
          });
          this.total_max_response_time.dataPoints.push({
            x: date,
            y: parseInt(record.total_max_response_time),
          });
        }
      }
      var theme = "light1";
      if (this.darkTheme) theme = "dark1";
      this.chart = new CanvasJS.Chart(this.test.id + "-chartContainer", {
        animationEnabled: true,
        zoomEnabled: true,
        //exportEnabled: true,
        theme: theme,
        axisX: {
          gridThickness: 0,
          lineThickness: 1,
        },
        axisY: {
          gridThickness: 0,
          lineThickness: 1,
        },
        legend: {
          cursor: "pointer",
          fontSize: 16,
          itemclick: this.toggleDataSeries,
        },
        toolTip: {
          shared: true,
        },
        data: [
          this.total_median_response_time,
          this.total_average_response_time,
          this.total_min_response_time,
          this.total_max_response_time,
        ],
      });
      this.chart.render();
    },
  },
  mounted() {
    this.setupChart();
  },
};
</script>

<style>
</style>