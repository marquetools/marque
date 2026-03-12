<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00050 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00401">
   <sch:p xmlns:ism="urn:us:gov:ic:ism"  ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00401][Error] EXDIS profiles requires ntk:ProfileDes with 
      type agencydissem (urn:us:gov:ic:ntk:profile:agencydissem).
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If ntk:AccessPolicy for the ntk:AccessProfile is EXDIS, then the ntk:ProfileDes must be agencydissem.
   </sch:p>
   <sch:rule context="ntk:AccessProfile[ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:xd']" id="ISM-ID-00401-R1">
      <sch:assert test="ntk:ProfileDes = 'urn:us:gov:ic:ntk:profile:agencydissem'" flag="error" role="error">
         [ISM-ID-00401][Error] EXDIS profiles requires ntk:ProfileDes with 
         type agencydissem (urn:us:gov:ic:ntk:profile:agencydissem).
      </sch:assert>
   </sch:rule>
</sch:pattern>
