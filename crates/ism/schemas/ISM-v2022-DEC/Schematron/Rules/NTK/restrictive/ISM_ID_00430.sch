<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="VALUECHECK"?>
<!-- Original rule id: NTK-ID-00037 -->
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00430">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
      [ISM-ID-00430][Error] Use of the restrictive access policy requires the Group and Individual Profile DES.
   </sch:p>
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If ntk:AccessProfile has an ntk:AccessPolicy = 'urn:us:gov:ic:aces:ntk:restrictive',
      then ntk:ProfileDes must be 'urn:us:gov:ic:ntk:profile:grp-ind'.
   </sch:p>
   <sch:rule id="ISM-ID-00430-R1" context="ntk:AccessProfile[ntk:AccessPolicy='urn:us:gov:ic:aces:ntk:restrictive']/ntk:ProfileDes">
      <sch:assert test=". = 'urn:us:gov:ic:ntk:profile:grp-ind'" flag="error" role="error">
         [ISM-ID-00430][Error] Use of the restrictive access policy requires the Group and Individual Profile DES.
      </sch:assert>
   </sch:rule>
</sch:pattern>
