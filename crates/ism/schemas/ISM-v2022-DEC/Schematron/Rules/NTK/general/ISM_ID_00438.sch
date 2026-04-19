<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00045 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron"  id="ISM-ID-00438" is-a="ValueExistsInList">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00438][Error] If an ntk:AccessProfileValue with @ntk:vocabulary of [datasphere:license] 
      is specified, then the value must exist in the LIC.CES License CVE (CVEnumLicLicense.xml).
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      For ntk:AccessProfileValue with vocabulary 'datasphere:license', invoke abstract rule ValueExistsInList
      to check if the value exists in the License CVE.</sch:p>
   <sch:param name="context" value="ntk:AccessProfileValue[@ntk:vocabulary='datasphere:license']"/>
   <sch:param name="list" value="$licenseList"/>
   <sch:param name="errMsg" value="'[ISM-ID-00438][Error] If an ntk:AccessProfileValue with @ntk:vocabulary of [datasphere:license]        is specified, then the value must exist in the LIC.CES License CVE (CVEnumLicLicense.xml)'"/>
</sch:pattern>
