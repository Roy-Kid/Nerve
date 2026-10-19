import Foundation
@main
struct HubProtocolTests {
 static func main() throws {
  let decoder=JSONDecoder();decoder.dateDecodingStrategy = .iso8601
  let data=try Data(contentsOf: URL(fileURLWithPath:"fixtures/hub-frame.json"))
  let frame=try decoder.decode(HubFrame.self,from:data)
  precondition(!frame.jobs.isEmpty)
  for job in frame.jobs { precondition(job.display != nil);precondition(job.status.rawValue == job.display!.status) }
  let old=Data("{\"jobs\":[]}".utf8)
  do { _ = try decoder.decode(HubFrame.self,from:old); fatalError("old protocol accepted") } catch {}
  var job=frame.jobs[0]
  let status=job.status
  job.outcome = .failure;job.health = .unresponsive
  precondition(job.status == status,"surface derived status from producer facets")
  precondition(frame.notifications.isEmpty)
  print("PASS hub projection decoding, version rejection, and surface authority boundary")
 }
}
