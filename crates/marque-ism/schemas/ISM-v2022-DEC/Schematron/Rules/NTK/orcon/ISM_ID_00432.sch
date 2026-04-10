<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00039 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00432">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00432][Error] ORCON requires the USA-Agency vocabulary (organization:usa-agency).
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If ntk:AccessPolicy for the ntk:AccessProfile is ORCON, then the vocabulary for the ntk:AccessProfileValue
      must be USA-Agency.
   </sch:p>
   <sch:rule id="ISM-ID-00432-R1" context="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:oc']/ntk:AccessProfileValue">
      <sch:assert test="@ntk:vocabulary = 'organization:usa-agency'" flag="error" role="error">
         [ISM-ID-00432][Error] ORCON requires the USA-Agency vocabulary (organization:usa-agency).</sch:assert>
   </sch:rule>
</sch:pattern>
