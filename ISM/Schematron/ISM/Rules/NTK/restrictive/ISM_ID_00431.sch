<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00038 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00431">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00431][Error] Use of the restrictive access policy requires a Group vocabulary type.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If ntk:AccessProfile has an ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:restrictive',
      then ntk:AccessProfileValue/@ntk:vocabulary must start with 'group:'.
   </sch:p>
   <sch:rule id="ISM-ID-00431-R1" context="ntk:AccessProfile[ntk:AccessPolicy='urn:us:gov:ic:aces:ntk:restrictive']/ntk:AccessProfileValue">
      <sch:assert test="starts-with(@ntk:vocabulary, 'group:')" flag="error" role="error">
         [ISM-ID-00431][Error] Use of the restrictive access policy requires a Group vocabulary type.
      </sch:assert>
   </sch:rule>
</sch:pattern>
