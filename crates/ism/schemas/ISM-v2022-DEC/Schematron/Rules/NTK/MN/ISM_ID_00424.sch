<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00031 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00424" is-a="ValidateTokenValuesExistenceInList">
   <sch:p xmlns:ism="urn:us:gov:ic:ism"  ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00424][Error] datasphere:mn:region vocabulary values must exist in the Mission Need Region CVE.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism"  ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      For AccessProfileValue with vocabulary 'datasphere:mn:region', invoke abstract rule ValueExistsInList
      to check if the value exists in the Mission Need Region CVE.
   </sch:p>
   <sch:param name="context" value="ntk:AccessProfileValue[@ntk:vocabulary='datasphere:mn:region']"/>
   <sch:param name="searchTermList" value="."/>
   <sch:param name="list" value="$regionList"/>
   <sch:param name="errMsg" value="'[ISM-ID-00424][Error] datasphere:mn:region vocabulary values must exist in the Mission Need Region CVE.'"/>
</sch:pattern>
