<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00030 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00423" is-a="ValidateTokenValuesExistenceInList" >
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00423][Error] datasphere:mn:issue vocabulary values must exist in the Mission Need Issue CVE.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      For AccessProfileValue with vocabulary 'datasphere:mn:issue', invoke abstract rule ValueExistsInList 
      to check if the value exists in the Mission Need Issue CVE.
   </sch:p>
   <sch:param name="context" value="ntk:AccessProfileValue[@ntk:vocabulary='datasphere:mn:issue']"/>
   <sch:param name="searchTermList" value="."/>
   <sch:param name="list" value="$issueList"/>
   <sch:param name="errMsg" value="'[ISM-ID-00423][Error] datasphere:mn:issue vocabulary values must exist in the Mission Need Issue CVE.'"/>
</sch:pattern>
