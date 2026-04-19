<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00035 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00428">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00428][Error] The @ntk:qualifier attribute value of either ‘originator’ or ‘dissemto’
      is required on every ntk:AccessProfileValue element for NTK Access Profiles based on the Agency Dissemination profile
      DES.</sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      Given an ntk:AccessProfile with an ntk:ProfileDes value of ‘urn:us:gov:ic:ntk:profile:agencydissem’, 
      one of ntk:AccessProfileValue/@qualifier='originator' or ntk:AccessProfileValue/@qualifier='dissemto' must exist.</sch:p>
   <sch:rule context="ntk:AccessProfile[ntk:ProfileDes='urn:us:gov:ic:ntk:profile:agencydissem']/ntk:AccessProfileValue" id="ISM-ID-00428-R1">
      <sch:assert test="@ntk:qualifier = 'originator' or @ntk:qualifier = 'dissemto'" flag="error" role="error">
         [ISM-ID-00428][Error] The @ntk:qualifier attribute value of either ‘originator’ or ‘dissemto’ is required 
         on every ntk:AccessProfileValue element for NTK Access Profiles based on the Agency Dissemination profile DES.
      </sch:assert>
   </sch:rule>
</sch:pattern>
