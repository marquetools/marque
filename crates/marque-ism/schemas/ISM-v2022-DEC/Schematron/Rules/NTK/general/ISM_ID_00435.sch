<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00042 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00435" is-a="ValueExistsInList">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00435][Error] If an access policy URN begins with ‘urn:us:gov:ic:aces:ntk’, the value
      must exist in the list of allowed values.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      For ntk:AccessPolicy that start with 'urn:us:gov:ic:aces:ntk:', invoke abstract rule ValueExistsInList
      to check if the value exists in the NTKAccessPolicy CVE.
   </sch:p>
   <sch:param name="context" value="ntk:AccessPolicy[starts-with(., 'urn:us:gov:ic:aces:ntk:')]"/>
   <sch:param name="list" value="$accessPolicyList"/>
   <sch:param name="errMsg" value="'[ISM-ID-00435][Error] Access Policy URNs that start with IC CIO reserved portion must exist in NTKAccessPolicy CVE.'"/>
</sch:pattern>
